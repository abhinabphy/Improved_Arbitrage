use dotenv::dotenv;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fmt::format;
use std::fs::File;
use std::io::Write;

mod engine;
mod executor;

use ArbEngine::datafetcher::data_fetcher;
use ArbEngine::engine::{Pool, Token, construct_network, find_arbitrage};
use ArbEngine::executor::execute_arbitrage;

/// -------------------------------
/// GraphQL response models
/// -------------------------------
#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
    #[serde(default)]
    locations: Option<Vec<GraphQLErrorLocation>>,
    #[serde(default)]
    path: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLErrorLocation {
    line: u32,
    column: u32,
}

#[derive(Debug, Deserialize)]
struct Data {
    #[serde(rename = "pairs")]
    pools: Vec<Pool>,
}

fn construct_graph_ofpools(pools: Vec<Pool>) {
    println!("Constructing graph with {} pools", pools.len());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let endpoint = "https://gateway.thegraph.com/api/subgraphs/id/A3Np3RQbaBA6oKJgiwDJeo5T3zrYfGHPWFYayMwtNDum";

    // let query = r#"
    // {
    //   pairs(
    //     first: 100,
    //     orderBy: reserveUSD,
    //     orderDirection: desc,
    //     where: {
    //       token0_: { derivedETH_gt: 0 },
    //       token1_: { derivedETH_gt: 0 }
    //     }
    //   ) {
    //     id
    //     reserveUSD
    //     reserve0
    //     reserve1
    //     token0 {
    //       id
    //       symbol
    //       name
    //       derivedETH
    //       decimals
    //     }
    //     token1 {
    //       id
    //       symbol
    //       name
    //       derivedETH
    //       decimals
    //     }
    //   }
    // }
    // "#;

    // let client = reqwest::Client::new();
    // dotenv().ok();
    // let api_key = env::var("GRAPH_API_KEY")?;
    // println!("{}", api_key);

    // let http_resp = client
    //     .post(endpoint)
    //     .header("Authorization",format!("Bearer {}", api_key))
    //     .json(&json!({ "query": query }))
    //     .send()
    //     .await?;

    // let status = http_resp.status();
    // println!("Response Status: {}", status);

    // let body = http_resp.text().await?;

    // if !status.is_success() {
    //     eprintln!("Non-200 response body: {}", body);
    //     return Err("Failed to fetch data (non-success HTTP status)".into());
    // }

    // // Deserialize GraphQL-style response
    // let gql: GraphQLResponse<Data> = serde_json::from_str(&body)?;

    // if let Some(errors) = &gql.errors {
    //     eprintln!("GraphQL returned errors:");
    //     for err in errors {
    //         eprintln!(" - {}", err.message);
    //     }
    //     return Err("GraphQL error; see logs".into());
    // }

    // let data = gql
    //     .data
    //     .ok_or_else(|| "GraphQL response had no `data` field".to_string())?;

    // println!("Fetched {} pools from subgraph", data.pools.len());

    // -----------------------------
    // Build network + find arb
    // -----------------------------
    // let network = construct_network(&data.pools);

    let pools = data_fetcher().await?;
    let network = construct_network(&pools);

    println!(
        "Constructed network with {} tokens and {} edges",
        network.tokens.len(),
        network.edges.len()
    );

    let arbitrages = find_arbitrage(&network, 0.0);
    println!("Found {} arbitrage opportunities", arbitrages.len());

    let cycles = arbitrages.clone();
    for arb in arbitrages {
        println!("{:#?}", arb);
    }

    execute_arbitrage(&cycles);

    Ok(())
}
