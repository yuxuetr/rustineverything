# Reth
Reth 是 Paradigm 主导的以太坊执行层客户端（Execution Client），从零用 Rust 写就，目标是高性能 + 模块化。

## 看点
- 全节点 / 归档节点，兼容以太坊主网与 L2
- 以 crate 形式拆分的模块化架构，可作为库复用
- 充分利用 Rust 并发与 MDBX 存储优化同步速度
- 活跃的 staking / MEV / 索引生态采用

## 适合参考
当你想了解一个生产级区块链客户端如何用 Rust 组织（网络、共识接口、状态存储、RPC）时，Reth 是难得的真实大型工程样本。
