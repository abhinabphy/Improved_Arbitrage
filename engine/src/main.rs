use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::Write;
 mod engine;
 mod executor;
use ArbEngine::engine::{Pool, Token, construct_network, find_arbitrage};
use ArbEngine::executor::execute_arbitrage;


#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    pools: Vec<Pool>,
}

// #[derive(Debug, Deserialize)]
// struct Pool {
//     id: String,
//     token0: Token,
//     token1: Token,
//     #[serde(rename = "feeTier")]
//     fee_tier: String,
//     #[serde(rename = "totalValueLockedUSD")]
//     total_value_locked_usd: String,
//     tick: String,
//     #[serde(rename = "sqrtPrice")]
//     sqrt_price: String,
// }

// #[derive(Debug, Deserialize)]
// struct Token {
//     symbol: String,
//     name: String,
//     id: String,
// }

fn construct_graph_ofpools(pools: Vec<Pool>) {

    
    // Placeholder for graph construction logic
    println!("Constructing graph with {} pools", pools.len());
}

fn export_network_json(tokens: &Vec<Token>, pools: &Vec<Pool>) -> std::io::Result<()> {
    let nodes: Vec<_> = tokens.iter()
        .map(|t| json!({"id": t.id, "label": t.symbol}))
        .collect();
    let edges: Vec<_> = pools.iter()
        .map(|p| json!({"source": p.token0.id, "target": p.token1.id, "id": p.id}))
        .collect();

    let graph_json = json!({ "nodes": nodes, "edges": edges });
    let mut file = File::create("network.json")?;
    file.write_all(graph_json.to_string().as_bytes())?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = "https://gateway.thegraph.com/api/subgraphs/id/DiYPVdygkfjDWhbxGSqAQxwBKmfKnkWQojqeM2rkLb3G";
    
    let query = r#"
    {
      pools(first: 500, orderBy: totalValueLockedUSD, orderDirection: desc) {
        id
        token0 { symbol name id decimals}
        token1 { symbol name id decimals}
        feeTier
        totalValueLockedUSD
        tick
        sqrtPrice
      }
    }
    "#;

    let client = reqwest::Client::new();
    
    let response = client
        .post(endpoint)
        .header("Authorization", "Bearer ad58cf9c17003146d9a16d553f5840d2")
        .json(&json!({
            "query": query
        }))
        .send()
        .await?;

    let result: GraphQLResponse;

    if response.status().is_success() {
        result = response.json().await?;
        println!("{:#?}", result.data.pools);
    } else {
        eprintln!("Error: {}", response.status());
        eprintln!("Response: {}", response.text().await?);
        return Err("Failed to fetch data".into());
    }
    
    let network = construct_network(&result.data.pools);
    // Export network to JSON for visualization
    export_network_json(&result.data.pools.iter().map(|p| p.token0.clone()).chain(result.data.pools.iter().map(|p| p.token1.clone())).collect(), &result.data.pools)?;
    println!("Constructed network with {} tokens and {} edges", network.tokens.len(), network.edges.len());
    let arbitrages = find_arbitrage(&network, 0.0);
    println!("Found {} arbitrage opportunities", arbitrages.len());
    let cycles = arbitrages.clone();
    for arb in arbitrages {
        //
        //map the arbtrage opportunities with usdc as start token
       if arb.start_token=="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"|| arb.start_token=="0xC02aaA39b223FE8D0A0E5C4F27eAD9083C756Cc2" {
        println!("{:#?}", arb);
    }
}
    //exec
    execute_arbitrage(&cycles);




    Ok(())
}