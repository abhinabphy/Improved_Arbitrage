use serde::Deserialize;
use core::panic;
use std::collections::{HashMap, HashSet};

/// -------------------------------
/// Data Models
/// -------------------------------
#[derive(Debug, Clone, Deserialize)]
pub struct Token {
    pub symbol: String,
    pub name: String,
    pub id: String,
    pub decimals: String

}

#[derive(Debug, Clone, Deserialize)]
pub struct Pool {
    pub id: String,
    pub token0: Token,
    pub token1: Token,
    #[serde(rename = "feeTier")]
    pub fee_tier: String,
    #[serde(rename = "totalValueLockedUSD")]
    pub total_value_locked_usd: String,
    pub tick: String,
    #[serde(rename = "sqrtPrice")]
    pub sqrt_price: String,
}

/// Directed edge between tokens (price-based)
#[derive(Debug, Clone)]
pub struct DirEdge {
    pub from: String,
    pub to: String,
    pub rate: f64,   // effective rate (includes fee)
    pub weight: f64, // -ln(rate)
    pub pool_id: String,
}

/// Token graph for arbitrage
#[derive(Debug, Clone)]
pub struct Network {
    pub tokens: Vec<String>,
    pub edges: Vec<DirEdge>,
}

/// Result of an arbitrage cycle
#[derive(Debug, Clone)]
pub struct ArbitrageCycle {
    pub start_token: String,
    pub path: Vec<String>,
    pub product: f64, // ∏ rate
    pub profit_pct: f64,
}

/// -------------------------------
/// 1️⃣ Construct network
/// -------------------------------
pub fn construct_network(pools: &Vec<Pool>) -> Network {
    let TWO_POW_96: f64 = 2.0f64.powi(96);
    let mut edges: Vec<DirEdge> = Vec::new();
    let mut token_set: HashSet<String> = HashSet::new();
    

    for p in pools {
//         if p.token0.id == "0x0000000000000000000000000000000000000000"
//     || p.token1.id == "0x0000000000000000000000000000000000000000" {
//     continue;
// }
        let sqrt_price_x96 = match p.sqrt_price.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if sqrt_price_x96 <= 0.0 {
            continue;
        }
        let decimals0 = match p.token0.decimals.parse::<i32>() {
            Ok(v) => v,
            Err(_) => panic!("Failed to parse decimals for token0 in pool {}", p.id),
        };
        let decimals1 = match p.token1.decimals.parse::<i32>() {
            Ok(v) => v,
            Err(_) => panic!("Failed to parse decimals for token1 in pool {}", p.id),
        };

let decimal_adjustment = 10f64.powi(decimals1 - decimals0);

        // compute price = (sqrtPriceX96 / 2^96)^2
        let price = (sqrt_price_x96 / TWO_POW_96).powi(2)*decimal_adjustment;

        // convert feeTier (e.g., "3000") to fraction (0.003)
        let fee = p
            .fee_tier
            .parse::<f64>()
            .map(|f| f / 1_000_000.0)
            .unwrap_or(0.003);

        // rates
        let rate_0to1 = (1.0 - fee) * price;
        let rate_1to0 = (1.0 - fee) / price;

        // add both directions
        if rate_0to1 > 0.0 && rate_0to1.is_finite() {
            edges.push(DirEdge {
                from: p.token0.id.clone(),
                to: p.token1.id.clone(),
                rate: rate_0to1,
                weight: -rate_0to1.ln(),
                pool_id: p.id.clone(),
            });
            token_set.insert(p.token0.id.clone());
            token_set.insert(p.token1.id.clone());
        }
        if rate_1to0 > 0.0 && rate_1to0.is_finite() {
            edges.push(DirEdge {
                from: p.token1.id.clone(),
                to: p.token0.id.clone(),
                rate: rate_1to0,
                weight: -rate_1to0.ln(),
                pool_id: p.id.clone(),
            });
        }
    }

    let mut tokens: Vec<String> = token_set.into_iter().collect();
    tokens.sort();

    Network { tokens, edges }
}

/// -------------------------------
/// 2️⃣ Arbitrage finder
/// -------------------------------
pub fn find_arbitrage(network: &Network, min_profit: f64) -> Vec<ArbitrageCycle> {
    let mut results = Vec::new();

    // Build adjacency list
    let mut adj: HashMap<String, Vec<&DirEdge>> = HashMap::new();
    for e in &network.edges {
        adj.entry(e.from.clone()).or_default().push(e);
    }

    // For each token as source, run Bellman-Ford in log-space
    for source in &network.tokens {
        let mut dist: HashMap<String, f64> = HashMap::new();
        let mut pred: HashMap<String, Option<String>> = HashMap::new();

        for t in &network.tokens {
            dist.insert(t.clone(), f64::INFINITY);
            pred.insert(t.clone(), None);
        }
        dist.insert(source.clone(), 0.0);

        let n = network.tokens.len();
        for _ in 0..n - 1 {
            let mut updated = false;
            for e in &network.edges {
                if let Some(&du) = dist.get(&e.from) {
                    if let Some(dv) = dist.get_mut(&e.to) {
                        if du + e.weight < *dv {
                            *dv = du + e.weight;
                            pred.insert(e.to.clone(), Some(e.from.clone()));
                            updated = true;
                        }
                    }
                }
            }
            if !updated {
                break;
            }
        }

        // detect negative cycle: product of rates > 1
        for e in &network.edges {
            if let (Some(&du), Some(&dv)) = (dist.get(&e.from), dist.get(&e.to)) {
                if du + e.weight < dv {
                    // reconstruct loop
                    let mut cycle_tokens = vec![e.to.clone()];
                    let mut cur = e.from.clone();
                    let mut visited = HashSet::new();
                    while !visited.contains(&cur) && cycle_tokens.len() < n {
                        visited.insert(cur.clone());
                        cycle_tokens.push(cur.clone());
                        if let Some(Some(prev)) = pred.get(&cur).map(|x| x.clone()) {
                            cur = prev;
                        } else {
                            break;
                        }
                    }
                    cycle_tokens.reverse();

                    // compute product (multiplier)
                    let mut product = 1.0;
                    for w in cycle_tokens.windows(2) {
                        if let Some(edges) = adj.get(&w[0]) {
                            if let Some(e) = edges.iter().find(|edge| edge.to == w[1]) {
                                product *= e.rate;
                            }
                        }
                    }
                    let profit_pct = (product - 1.0) * 100.0;
                    if profit_pct > min_profit * 100.0 {
                        results.push(ArbitrageCycle {
                            start_token: source.clone(),
                            path: cycle_tokens.clone(),
                            product,
                            profit_pct,
                        });
                    }
                }
            }
        }
    }

    results
}
