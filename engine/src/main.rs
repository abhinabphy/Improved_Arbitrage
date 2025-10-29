use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;

mod engine;
use engine::{Pool, Token, construct_network, find_arbitrage};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = "https://gateway.thegraph.com/api/subgraphs/id/DiYPVdygkfjDWhbxGSqAQxwBKmfKnkWQojqeM2rkLb3G";
    
    let query = r#"
    {
      pools(first: 1000, orderBy: totalValueLockedUSD, orderDirection: desc) {
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
    println!("Constructed network with {} tokens and {} edges", network.tokens.len(), network.edges.len());
    let arbitrages = find_arbitrage(&network, 0.0);
    println!("Found {} arbitrage opportunities", arbitrages.len());
    for arb in arbitrages {
        println!("{:#?}", arb);
    }



    Ok(())
}