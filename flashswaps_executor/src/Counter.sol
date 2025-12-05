// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;


import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import '@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol';
import '@uniswap/v2-periphery/contracts/interfaces/IUniswapV2Router02.sol';





contract flashswap is FlashLoanSimpleReceiverBase{
    address public owner;
    IUniswapV2Router02 constant router=IUniswapV2Router02(0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D); //uniswap v2 router
    IPoolAddressesProvider constant provider=IPoolAddressesProvider(0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e); //aave v3 mainnet provider
   
    struct ArbParams {
        address startToken;
        address[] path;
        uint256 minProfit;     
    }

    constructor() FlashLoanSimpleReceiverBase(provider) {
        owner = msg.sender;
    }

    modifier onlyOwner {
        require(msg.sender == owner, "Not owner");
        _;

    }

    function createflashloan(address asset,uint amount,bytes memory params) public onlyOwner{
        address receiverAddress = address(this);
        uint16 referralCode = 0;

        POOL.flashLoanSimple(
            receiverAddress,
            asset,
            amount,
            params,
            referralCode
        );
    }
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override  returns (bool) {
        // This contract now has the funds requested.
        // Your logic goes here.
        ArbParams memory p=abi.decode(params,(ArbParams));
       // require(address(initiator)==address(this),"not initiated by this contract");

       //minimum case is a triangular arbitrage
       // require(p.path.length >= 2, "path too short");
        //require(p.path[0] == asset, "path must start with asset");
        //require(p.path[p.path.length - 1] == asset, "path must end with asset");

        uint256 startBalance = IERC20(asset).balanceOf(address(this));
        IERC20(asset).approve(address(router), type(uint256).max);
        router.swapExactTokensForTokens(
            amount,
            0,// end checking for profit make this input field meaningless
            p.path,
            address(this),
            block.timestamp+3000
        );

        uint256 endBalance= IERC20(asset).balanceOf(address(this));

        //require(endBalance >= startBalance + premium, "Not enough profit");



        

        uint256 totalDebt = amount+ premium;

        // Approve the Pool contract allowance to *pull* the owed amount
        IERC20(asset).approve(address(POOL), totalDebt);

        return true;
    }

    receive() external payable {}



}
