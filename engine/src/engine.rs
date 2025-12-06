use serde::Deserialize;
use std::collections::{HashMap, HashSet};
/// -------------------------------
/// Data Models (from subgraph)
/// -------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct Token {
    pub symbol: String,
    pub name: String,
    pub id: String,
    pub decimals: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pool {
    pub id: String,
    pub token0: Token,
    pub token1: Token,
    pub reserve0: String,              // from subgraph: already scaled (not raw 1e18)
    pub reserve1: String,              // from subgraph: already scaled
    #[serde(default)]
    pub reserveUSD: Option<String>,    // optional, if you add it to the query
}

/// Directed edge between tokens (price-based)
#[derive(Debug, Clone)]
pub struct DirEdge {
    pub from: String,
    pub to: String,
    pub rate: f64,   // effective small-trade rate (includes fee)
    pub weight: f64, // -ln(rate)
    pub pool_id: String,
}

/// Token graph for arbitrage (external API type)
#[derive(Debug, Clone)]
pub struct Network {
    pub tokens: Vec<String>, // token IDs
    pub edges: Vec<DirEdge>, // logical edges (string-based)
}

/// Result of an arbitrage cycle
#[derive(Debug, Clone)]
pub struct ArbitrageCycle {
    pub start_token: String,
    pub path: Vec<String>,  // token IDs in cycle order
    pub product: f64,       // ∏ rate along cycle
    pub profit_pct: f64,    // (product - 1) * 100
}

/// -------------------------------
/// Internal integer-indexed graph
/// -------------------------------
#[derive(Debug, Clone)]
struct IndexedEdge {
    from: usize,
    to: usize,
    rate: f64,
    weight: f64,
    pool_id: String,
}

#[derive(Debug, Clone)]
struct IndexedNetwork<'a> {
    tokens: &'a [String],           // index -> token id (borrow from Network)
    edges: Vec<IndexedEdge>,        // integer indexed edges
    adj: Vec<Vec<usize>>,           // adjacency: node -> list of edge indices
}

/// Build integer-index network from string-based Network
fn index_network(network: &Network) -> IndexedNetwork<'_> {
    let n = network.tokens.len();
    let mut token_to_idx = HashMap::<String, usize>::with_capacity(n);
    for (i, t) in network.tokens.iter().enumerate() {
        token_to_idx.insert(t.clone(), i);
    }

    let mut edges: Vec<IndexedEdge> = Vec::with_capacity(network.edges.len());
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

    for e in &network.edges {
        let from_idx = match token_to_idx.get(&e.from) {
            Some(&idx) => idx,
            None => continue,
        };
        let to_idx = match token_to_idx.get(&e.to) {
            Some(&idx) => idx,
            None => continue,
        };

        let edge_index = edges.len();
        edges.push(IndexedEdge {
            from: from_idx,
            to: to_idx,
            rate: e.rate,
            weight: e.weight,
            pool_id: e.pool_id.clone(),
        });
        adj[from_idx].push(edge_index);
    }

    IndexedNetwork {
        tokens: &network.tokens,
        edges,
        adj,
    }
}

/// ------------------------------------------------------------
/// 1️⃣ Construct network (Uniswap V2, TVL-filtered, no decimals)
/// ------------------------------------------------------------
pub fn construct_network(pools: &Vec<Pool>) -> Network {
    let mut edges: Vec<DirEdge> = Vec::new();
    let mut token_set: HashSet<String> = HashSet::new();

    // Uniswap V2 fee
    let fee = 0.003_f64;
    let one_minus_fee = 1.0 - fee;

    // simple TVL cutoff (can tune or remove if you want everything)
    const MIN_TVL_USD: f64 = 10_000.0;

    for p in pools {
        // Skip zero-address garbage
        if p.token0.id == "0x0000000000000000000000000000000000000000"
            || p.token1.id == "0x0000000000000000000000000000000000000000"
        {
            continue;
        }

        // Parse reserves (already scaled from subgraph)
        let reserve0: f64 = match p.reserve0.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let reserve1: f64 = match p.reserve1.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        if reserve0 <= 0.0 || reserve1 <= 0.0 {
            continue;
        }

        // Optional TVL filter if reserveUSD is present
        if let Some(s) = &p.reserveUSD {
            if let Ok(tvl) = s.parse::<f64>() {
                if tvl < MIN_TVL_USD {
                    continue;
                }
            }
        }

        // Uniswap V2 mid-price: token0 → token1
        // reserves are already in real token units, so no decimal adjustment.
        let mid_price_0to1 = reserve1 / reserve0;

        if !mid_price_0to1.is_finite() || mid_price_0to1 <= 0.0 {
            continue;
        }

        // Small-trade effective rates including fee (approximate)
        let rate_0to1 = one_minus_fee * mid_price_0to1;
        let rate_1to0 = one_minus_fee / mid_price_0to1;

        if !(rate_0to1.is_finite() && rate_0to1 > 0.0) {
            continue;
        }
        if !(rate_1to0.is_finite() && rate_1to0 > 0.0) {
            continue;
        }

        // Add both directions
        edges.push(DirEdge {
            from: p.token0.id.clone(),
            to: p.token1.id.clone(),
            rate: rate_0to1,
            weight: -rate_0to1.ln(),
            pool_id: p.id.clone(),
        });

        edges.push(DirEdge {
            from: p.token1.id.clone(),
            to: p.token0.id.clone(),
            rate: rate_1to0,
            weight: -rate_1to0.ln(),
            pool_id: p.id.clone(),
        });

        token_set.insert(p.token0.id.clone());
        token_set.insert(p.token1.id.clone());
    }

    let mut tokens: Vec<String> = token_set.into_iter().collect();
    tokens.sort();

    Network { tokens, edges }
}

/// ------------------------------------------------------------
/// 2️⃣ Arbitrage finder (integer-index Bellman–Ford in log-space)
/// ------------------------------------------------------------
/// min_profit is in fractional form: 0.01 = 1%, 0.001 = 0.1%, etc.
pub fn find_arbitrage(network: &Network, min_profit: f64) -> Vec<ArbitrageCycle> {
    let idx_net = index_network(network);
    let n = idx_net.tokens.len();
    let m = idx_net.edges.len();
    let mut results = Vec::new();

    if n == 0 || m == 0 {
        return results;
    }

    // dist and pred indexed by node (super-source: all zeros)
    let mut dist = vec![0.0_f64; n];
    let mut pred = vec![usize::MAX; n];

    // Bellman–Ford: V-1 relaxations over all edges
    for _ in 0..(n - 1) {
        let mut updated = false;
        for e in &idx_net.edges {
            let du = dist[e.from];
            let dv = dist[e.to];
            let cand = du + e.weight;
            if cand < dv {
                dist[e.to] = cand;
                pred[e.to] = e.from;
                updated = true;
            }
        }
        if !updated {
            break;
        }
    }

    // Detect negative cycles on N-th iteration
    let mut seen_cycle_node = vec![false; n];

    for e in &idx_net.edges {
        let du = dist[e.from];
        let dv = dist[e.to];
        if du + e.weight < dv {
            let cycle_node = e.to;
            if seen_cycle_node[cycle_node] {
                continue;
            }
            seen_cycle_node[cycle_node] = true;

            let cycle_indices = reconstruct_cycle_indices(cycle_node, &pred);
            if cycle_indices.len() < 3 {
                continue;
            }

            // sanity: ensure it's a closed loop
            let mut cycle = cycle_indices.clone();
            if *cycle.first().unwrap() != *cycle.last().unwrap() {
                cycle.push(*cycle.first().unwrap());
            }

            // optional: avoid insanely long cycles
            if cycle.len() > 10 {
                continue;
            }

            // compute product along cycle
            let product = cycle_product(&cycle, &idx_net);
            let profit_pct = (product - 1.0) * 100.0;

            if profit_pct > min_profit * 100.0 && product.is_finite() && product > 1.0 {
                // map indices back to token IDs
                let path = cycle
                    .iter()
                    .map(|&i| idx_net.tokens[i].clone())
                    .collect::<Vec<_>>();

                results.push(ArbitrageCycle {
                    start_token: path[0].clone(),
                    path,
                    product,
                    profit_pct,
                });
            }
        }
    }

    results
}

/// Reconstruct a negative cycle in terms of node indices.
///
/// Standard trick:
/// 1. Walk pred n times from `start` to ensure you land inside the cycle.
/// 2. Then walk until you come back to the starting node.
fn reconstruct_cycle_indices(start: usize, pred: &[usize]) -> Vec<usize> {
    let n = pred.len();
    let mut v = start;

    // Step into the cycle
    for _ in 0..n {
        if pred[v] == usize::MAX {
            return Vec::new();
        }
        v = pred[v];
    }

    // v is now guaranteed to be in a cycle
    let cycle_start = v;
    let mut cycle = vec![cycle_start];
    let mut cur = pred[cycle_start];

    while cur != cycle_start {
        cycle.push(cur);
        cur = pred[cur];
    }
    cycle.push(cycle_start); // close the loop
    cycle
}

/// Compute ∏ rate along cycle indices, using adjacency.
fn cycle_product(cycle: &[usize], net: &IndexedNetwork<'_>) -> f64 {
    let mut product = 1.0;

    for w in cycle.windows(2) {
        let u = w[0];
        let v = w[1];

        // find edge u -> v
        let ei = match net.adj[u].iter().find(|&&ei| net.edges[ei].to == v) {
            Some(&ei) => ei,
            None => return 1.0, // inconsistent graph; treat as no-arb
        };

        product *= net.edges[ei].rate;
        if !product.is_finite() || product <= 0.0 {
            return 1.0;
        }
    }

    product
}
