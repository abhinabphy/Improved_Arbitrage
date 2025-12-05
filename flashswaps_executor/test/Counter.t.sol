// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {flashswap} from "../src/Counter.sol";

import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import '@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol';
import '@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol';

contract CounterTest is Test {
    flashswap public multihop;



    function setUp() public {
        multihop = new flashswap();
    }

    function test_Owner() public {
        assertEq(multihop.owner(), address(this));
    }
// ArbitrageCycle {
//     start_token: "0xfdc9d2a3cae56e484a85de3c2e812784a8184d0d",
//     path: [
//         "0xe2fc85bfb48c4cf147921fbe110cf92ef9f26f94",
//         "0xdac17f958d2ee523a2206206994597c13d831ec7",
//         "0xfa704148d516b209d52c2d75f239274c8f8eaf1a",
//     ],
//     product: 2.5994892236730062e23,
//     profit_pct: 2.599489223673006e25,
// }
    function test_CreateFlashloan() public {
    
    address asset = 0xdAC17F958D2ee523a2206206994597C13D831ec7;

    address[] memory path = new address[](4);
    path[0] = asset;
    path[1] = 0xE2Fc85BfB48C4cF147921fBE110cf92Ef9f26F94;
    path[2] = 0xdAC17F958D2ee523a2206206994597C13D831ec7;
    path[3] = asset;
     bytes memory params = abi.encode(path[0], path, 1e6);
    // call: multihop.createflashloan(asset, 1_000_000n, params)
        // Just test that it doesn't revert
    multihop.createflashloan(asset, 1e6, params);
    }
// ArbitrageCycle {
//     start_token: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
//     path: [
//         "0xd0ec028a3d21533fdd200838f39c85b03679285d",
//         "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
//         "0x40e3d1a4b2c47d9aa61261f5606136ef73e28042",
//     ],
//     product: 1.3683518713700848,
//     profit_pct: 36.83518713700848,
// }
function  test_arbitrage() public {
 
    address asset = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48; // USDC

    address[] memory path = new address[](5);
    path[0] = asset;
    path[1] = 0xD0eC028a3D21533Fdd200838F39c85B03679285D;
    path[2] = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    path[3] = 0x40e3d1A4B2C47d9AA61261F5606136ef73E28042;
    path[4]=  asset;

    //bytes memory params = abi.encode(asset, path, 1e6);
    flashswap.ArbParams memory p = flashswap.ArbParams({ startToken: asset, path: path, minProfit: 1e6 });
    bytes memory params = abi.encode(p);

    multihop.createflashloan(asset, 1e6, params);
}

function test_router() public {
   // mint weth to this contract
    address asset = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2; // WETH
    deal(asset, address(this), 10e9); 
    uint256 initial_balance_weth = IERC20(asset).balanceOf(address(this));
    IUniswapV2Router02 router = IUniswapV2Router02(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D);

    address[] memory path = new address[](2);
    path[0] = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2; 
    path[1] = 0xD13cfD3133239a3c73a9E535A5c4DadEE36b395c; 
    IERC20(asset).approve(address(router), type(uint256).max);

    // router.swapExactTokensForTokens(
    //     1e9,
    //     0, // accept any output, we just want the call to work
    //     path,
    //     address(this),
    //     block.timestamp + 300
    // );
     flashswap.ArbParams memory p = flashswap.ArbParams({ startToken: asset, path: path, minProfit: 1e6 });
    bytes memory params = abi.encode(p);

    multihop.createflashloan(asset, 1e6, params);

   uint256 final_balance_weth = IERC20(path[0]).balanceOf(address(this));
    assert(final_balance_weth < initial_balance_weth);

}
}
