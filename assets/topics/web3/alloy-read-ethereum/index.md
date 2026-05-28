---
title: "用 alloy 读以太坊链上状态"
description: "alloy 取代 ethers-rs 后的新 API：连接节点、查询余额、读合约、解析事件。"
date: "2026-05-23"
subtopic: "evm"
tags: ["alloy", "evm", "ethereum"]
---

# 用 alloy 读以太坊链上状态

[alloy](https://github.com/alloy-rs/alloy) 是 `ethers-rs` 的继任者（ethers 已停止维护）。它把 provider、签名、ABI 编解码、类型系统重新做了一遍，模块化更彻底、性能更好。如果你还在用 ethers，迁移到 alloy 是当前推荐路径。

## 连接节点

`Provider` 是与链交互的入口。连到一个 RPC endpoint：

```rust
use alloy::providers::{Provider, ProviderBuilder};

let rpc_url = "https://eth.llamarpc.com".parse()?;
let provider = ProviderBuilder::new().on_http(rpc_url);

let block = provider.get_block_number().await?;
println!("最新区块: {block}");
```

## 查询余额

地址用强类型 `Address`，金额用 `U256`（256 位无符号整数，对应 EVM word）：

```rust
use alloy::primitives::{address, utils::format_ether};

let who = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"); // vitalik.eth
let wei = provider.get_balance(who).await?;
println!("余额: {} ETH", format_ether(wei));
```

`U256` 上的算术不会静默溢出——这正是 Rust 类型系统在金融场景的价值：单位（wei vs ether）和溢出都在类型层面被约束。

## 读合约：sol! 宏

alloy 用 `sol!` 宏从 Solidity 接口直接生成 Rust 绑定，编译期就有类型安全的方法：

```rust
use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address owner) external view returns (uint256);
        function symbol() external view returns (string);
    }
}

let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
let erc20 = IERC20::new(usdc, &provider);

let symbol = erc20.symbol().call().await?._0;
let bal = erc20.balanceOf(who).call().await?._0;
println!("{who} 持有 {bal} {symbol}");
```

`sol!` 把 ABI 编解码全部在编译期生成，没有运行时字符串拼 ABI 的脆弱性。

## 解析事件日志

监听 / 回溯事件用 filter + 解码：

```rust
sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
}

let filter = Filter::new()
    .address(usdc)
    .event_signature(Transfer::SIGNATURE_HASH)
    .from_block(BlockNumberOrTag::Latest);

let logs = provider.get_logs(&filter).await?;
for log in logs {
    let decoded = log.log_decode::<Transfer>()?;
    let t = decoded.inner.data;
    println!("{} → {} : {}", t.from, t.to, t.value);
}
```

## 心智模型

alloy 把"链上交互"拆成正交的几块：

1. **primitives**：`Address`、`U256`、`B256` 等强类型；
2. **provider**：RPC 传输与查询；
3. **sol!**：从 Solidity 接口生成类型安全绑定；
4. **signer**：本地 / 硬件 / KMS 签名（写交易时用）。

读链只需要 provider + sol!；要发交易再加 signer。下一步可以看 [revm](https://github.com/bluealloy/revm)——一个纯 Rust 的 EVM，能在本地模拟交易、做 gas 估算或写 MEV 工具。
