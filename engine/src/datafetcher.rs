use std::sync::Arc;
//import time in seconds
use std::time::Instant;

use alloy::{
    primitives::{Address, U256, address},
    providers::{Provider, ProviderBuilder, WsConnect, bindings::IMulticall3::IMulticall3Calls},
    signers::k256::elliptic_curve::pkcs8::der,
    sol,
    sol_types::SolCall,
};
use dashmap::DashMap;
use eyre::Result;
// use eyre::Result;
use tokio::sync::Semaphore;

use crate::engine::{Pool, Token};

sol! {
    #[sol(rpc)]
    interface IUniswapV2Factory {
        function allPairsLength() returns (uint);
        function allPairs(uint) returns (address);
    }

    #[sol(rpc)]
    interface IUniswapV2Pair {
        function token0() returns (address);
        function token1() returns (address);
        function getReserves()
            returns (uint112 reserve0, uint112 reserve1, uint32);
    }
    #[sol(rpc)]
    interface IERC20Metadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }
    struct Call {
        address target;
        bytes callData;
    }

    #[sol(rpc)]
    interface IMulticall2 {
        function aggregate(Call[] calls)
            returns (uint256, bytes[] returnData);
    }
}

const UNISWAP_V2_FACTORY: Address = address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");
const MULTICALL2: Address = address!("0x5ba1e12693dc8f9c48aad8770482f4739beed696");

//#[tokio::main]
pub async fn data_fetcher() -> Result<Vec<Pool>> {
    let rpc_url = "https://reth-ethereum.ithaca.xyz/rpc".parse()?;
    let provider = ProviderBuilder::new().connect_http(rpc_url);

    let provider = Arc::new(provider);

    let factory = IUniswapV2Factory::new(UNISWAP_V2_FACTORY, provider.clone());
    let token_cache: Arc<DashMap<Address, Token>> = Arc::new(DashMap::new());

    let pairs_len: U256 = factory.allPairsLength().call().await?;
    println!("Total pairs: {}", pairs_len);
    let mut pools: Vec<Pool> = Vec::new();

    let max = pairs_len.min(U256::from(20));
    const ADDR_BATCH: usize = 30;
    const MAX_CONCURRENCY: usize = 3;

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));
    //let mut tasks = Vec::new();

    let mut i = U256::ZERO;
    while i < U256::from(1) {
        let pair_addr: Address = factory.allPairs(i).call().await?;
        let pair = IUniswapV2Pair::new(pair_addr, provider.clone());

        let token0_addr: Address = pair.token0().call().await?;
        let token1_addr: Address = pair.token1().call().await?;
        let token0 = load_token(token0_addr, provider.clone(), &token_cache).await?;
        let token1 = load_token(token1_addr, provider.clone(), &token_cache).await?;

        let reserves = pair.getReserves().call().await?;
        let r0: U256 = U256::from(reserves.reserve0);
        let r1: U256 = U256::from(reserves.reserve1);
        let pool = Pool {
            id: pair_addr.to_string(),
            token0,
            token1,
            reserve0: r0.to_string(),
            reserve1: r1.to_string(),
            reserveUSD: Some(U256::ZERO.to_string()), // Placeholder, will be calculated later
        };
        i += U256::ONE;
        println!("{:#?}", token_cache);
        pools.push(pool);
    }

    Ok(pools)
}
// helper to load Token from an on-chain ERC20
async fn load_token<P>(
    addr: Address,
    provider: Arc<P>,
    cache: &DashMap<Address, Token>,
) -> Result<Token>
where
    P: Provider + 'static,
{
    if let Some(token) = cache.get(&addr) {
        return Ok(token.clone());
    }

    let erc20 = IERC20Metadata::new(addr, provider);
    let name: String = erc20.name().call().await?;
    let symbol: String = erc20.symbol().call().await?;
    let decimals_u8: u8 = erc20.decimals().call().await?;
    let token = Token {
        id: format!("{:#x}", addr),
        name,
        symbol,
        decimals: decimals_u8.to_string(),
    };
    cache.insert(addr, token.clone());
    Ok(token)
}
//another helper function to build the multicall
fn build_all_pairs_calls(start: U256, count: usize, factory: Address) -> Vec<Call> {
    (0..count)
        .map(|i| Call {
            target: factory,
            callData: IUniswapV2Factory::allPairsCall(start + U256::from(i))
                .abi_encode()
                .into(),
        })
        .collect()
}
//helper fucntion to fetch all pools via multicall
async fn fetch_pools<p>(provider: Arc<p>, factory: Address, count: usize) -> Result<Vec<Address>>
where
    p: Provider + 'static,
{
    let mut all_pairs = Vec::new();
    let mut start = U256::ZERO;
    while start < count {
        let calls = build_all_pairs_calls(start, count, factory);
        let multicall = IMulticall2::new(MULTICALL2, provider.clone());
        let aggregate_result = multicall.aggregate(calls).call().await?;
        let returndata = aggregate_result.returnData;
        for raw in returndata {
            let bytes: &[u8] = &raw;
            let pair_addr: Address = IUniswapV2Factory::allPairsCall::abi_decode_returns(bytes)?;
            println!("{:?}", pair_addr);
        }
        start += U256::ONE;
    }
    Ok(all_pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_data_fetcher() {
        let now = Instant::now();

        let pools = data_fetcher().await.unwrap();
        assert!(!pools.is_empty());
        for pool in pools.iter().take(5) {
            println!("{:?}", pool);
        }
        let t = now.elapsed();
        println!("Time taken: {} seconds", t.as_secs());
    }

    //test building multicall calldata
    #[tokio::test]
    async fn test_build_all_pairs_calls() {
        let factory = UNISWAP_V2_FACTORY;
        let calls = build_all_pairs_calls(U256::from(0), 5, factory);
        assert_eq!(calls.len(), 5);
        for (i, call) in calls.iter().enumerate() {
            let expected_data = IUniswapV2Factory::allPairsCall(U256::from(i as u64)).abi_encode();
            assert_eq!(call.target, factory);
            println!("Call {} data: {:?}", i, call.callData);
        }
    }

    #[tokio::test]
    async fn test_poolfetcher() -> Result<(), Box<dyn std::error::Error>> {
        let rpc_url = "https://reth-ethereum.ithaca.xyz/rpc".parse()?;

        let provider = ProviderBuilder::new().connect_http(rpc_url);

        let provider = Arc::new(provider);
        let factory = UNISWAP_V2_FACTORY;
        let result = fetch_pools(provider, factory, 10).await;

        match result {
            Ok(pools) => {
                println!("Fetched pools: {:?}", pools);
            }
            Err(e) => {
                eprintln!("Error fetching pools: {:?}", e);
            }
        }
        Ok(())
    }
}
