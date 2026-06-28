# 中文文案字典（方案 A：编译期内嵌，app_core 同步查表）
# 格式：key = value（# 注释，点号分隔的 key）。en.ftl 必须键集合一致。

# ── 导航 ──
nav.blog = 博客
nav.podcast = 播客
nav.forum = 论坛
nav.cases = 案例
nav.start = 开始学习
nav.embedded = 嵌入式
nav.admin = 🛡️ 管理后台

# ── 登录 / 用户 ──
auth.sign_in = 登录
auth.sign_in_desc = 选择一种方式继续
auth.continue_with = 继续
auth.terms = 登录即表示你同意我们的服务条款和隐私政策
auth.logout = 退出登录
user.my_topics = 我的话题
user.my_annotations = 我的标注

# ── 博客页 ──
blog.title = 博客
blog.subtitle = 探索 Rust 的无限可能
blog.filter = 标签筛选
blog.all = 全部
blog.empty = 暂无文章
blog.articles = 文章
blog.no_results = 没有匹配该标签的文章

# ── 页脚 ──
footer.tagline = 专注 Rust 技术栈

# ── 首页 Hero ──
hero.title = 专注 Rust 技术栈的学习与实战
hero.subtitle = 文档、博客、课程、案例一站式聚合：AI、后端、前端、跨端、Web3、Wasm、嵌入式、命令行等。
hero.btn.docs = 进入文档
hero.btn.blog = 浏览博客
hero.btn.cases = 查看案例
hero.stat.docs.value = 文档
hero.stat.docs.label = 从零到一
hero.stat.blog.value = 博客
hero.stat.blog.label = 持续更新
hero.stat.course.value = 课程
hero.stat.course.label = 系统学习
hero.stat.cases.value = 案例
hero.stat.cases.label = 可复用模板

# ── 首页板块卡片 ──
home.section_title = 专注 Rust 生态
home.section_subtitle = 从底层原理到全栈实战，构建高性能、高可靠的软件系统
home.card.basics.title = Rust 基础
home.card.basics.desc = 深入浅出所有权、生命周期、Trait 等核心概念。
home.card.fullstack.title = 全栈开发
home.card.fullstack.desc = 使用 Dioxus, Axum, SeaORM 快速构建跨平台应用。
home.card.aiwasm.title = AI 与 WASM
home.card.aiwasm.desc = 探索 WebAssembly 高性能计算与 Rust AI 生态。

# ── 板块共享 UI ──
board.all = 全部
board.search = 搜索文章 / 标签…
board.featured = 精选 crate
board.empty = 暂无文章。
board.back_prefix = ← 返回
board.load_error_prefix = 加载失败：

# ── 嵌入式 ──
embedded.label = 嵌入式
embedded.tagline = 用 Rust 写裸机与实时系统：no_std、Embassy、RTIC 与主流 MCU 平台。
embedded.sub.no-std.label = no_std
embedded.sub.no-std.blurb = 脱离标准库，在没有操作系统的目标上运行 Rust。
embedded.sub.embassy.label = Embassy
embedded.sub.embassy.blurb = 嵌入式异步运行时，用 async/await 写中断驱动的固件。
embedded.sub.rtic.label = RTIC
embedded.sub.rtic.blurb = 基于硬件优先级的并发框架，零成本任务调度。
embedded.sub.hal.label = HAL / PAC
embedded.sub.hal.blurb = embedded-hal 抽象层与外设访问 crate，跨芯片复用驱动。
embedded.sub.defmt.label = 日志与调试
embedded.sub.defmt.blurb = defmt 高效日志 + probe-rs 烧录调试工作流。
embedded.sub.platforms.label = 平台
embedded.sub.platforms.blurb = RP2040 / STM32 / ESP32 / nRF 等主流 MCU 平台实践。
embedded.crate.embassy.blurb = 嵌入式异步运行时与 HAL
embedded.crate.rtic.blurb = 实时中断驱动并发框架
embedded.crate.embedded-hal.blurb = 跨平台外设抽象 trait
embedded.crate.defmt.blurb = 嵌入式高效结构化日志
embedded.crate.probe-rs.blurb = 烧录与调试工具链
embedded.crate.heapless.blurb = 无堆分配的静态容量数据结构

# ── AI ──
ai.label = AI
ai.tagline = 用 Rust 做张量计算、模型推理与 LLM 应用：candle、burn 与 ONNX 生态。
ai.sub.tensors.label = 张量计算
ai.sub.tensors.blurb = 在 CPU / CUDA / Metal 上做张量运算与自动微分。
ai.sub.inference.label = 推理引擎
ai.sub.inference.blurb = 加载预训练权重做前向推理，部署到服务端或边缘。
ai.sub.llm.label = 大模型
ai.sub.llm.blurb = 本地跑 LLM、量化、KV cache 与流式生成。
ai.sub.tokenizers.label = 分词
ai.sub.tokenizers.blurb = BPE / WordPiece 分词与 HuggingFace tokenizers。
ai.sub.training.label = 训练框架
ai.sub.training.blurb = 用纯 Rust 框架定义网络、反向传播与优化器。
ai.sub.embeddings.label = 向量与检索
ai.sub.embeddings.blurb = 句向量、相似度检索与向量数据库集成。
ai.crate.candle.blurb = HuggingFace 极简张量与推理框架
ai.crate.burn.blurb = 纯 Rust、多后端深度学习框架
ai.crate.tch.blurb = libtorch（PyTorch C++）绑定
ai.crate.tokenizers.blurb = HuggingFace 高性能分词器
ai.crate.ort.blurb = ONNX Runtime 的 Rust 绑定
ai.crate.safetensors.blurb = 安全、零拷贝的张量序列化格式

# ── Web3 ──
web3.label = Web3
web3.tagline = 区块链与去中心化应用的 Rust 工具链：以太坊、Solana、Substrate 与智能合约。
web3.sub.evm.label = 以太坊 / EVM
web3.sub.evm.blurb = 用 alloy 读写链上状态、发交易、解析事件日志。
web3.sub.solana.label = Solana
web3.sub.solana.blurb = Solana 程序与客户端开发，高吞吐链上逻辑。
web3.sub.substrate.label = Substrate
web3.sub.substrate.blurb = 用 polkadot-sdk 搭建自定义区块链与 runtime pallet。
web3.sub.contracts.label = 智能合约
web3.sub.contracts.blurb = ink! / Solana 程序 / EVM 字节码与合约交互。
web3.sub.wallet.label = 钱包与签名
web3.sub.wallet.blurb = 密钥管理、签名方案与交易构造。
web3.sub.indexing.label = 链上索引
web3.sub.indexing.blurb = 区块/事件扫描、状态重建与数据服务。
web3.crate.alloy.blurb = 以太坊互操作的现代 Rust 工具集
web3.crate.revm.blurb = 高性能纯 Rust EVM 实现
web3.crate.solana-sdk.blurb = Solana 链上程序与客户端 SDK
web3.crate.anchor.blurb = Solana 程序开发框架
web3.crate.polkadot-sdk.blurb = Substrate / Polkadot 区块链框架
web3.crate.ink.blurb = 面向 Substrate 的智能合约 eDSL

# ── WASM ──
wasm.label = WASM
wasm.tagline = WebAssembly 全景：浏览器互操作、WASI、组件模型与服务端运行时。
wasm.sub.bindgen.label = wasm-bindgen
wasm.sub.bindgen.blurb = Rust 与 JS 互调，把 Rust 编进浏览器。
wasm.sub.wasi.label = WASI
wasm.sub.wasi.blurb = WebAssembly 系统接口：文件、时钟、网络的可移植 ABI。
wasm.sub.components.label = 组件模型
wasm.sub.components.blurb = WIT / wit-bindgen 与可组合的 wasm 组件。
wasm.sub.runtimes.label = 运行时
wasm.sub.runtimes.blurb = wasmtime / wasmer 在服务端嵌入 wasm 沙箱。
wasm.sub.frontend.label = 前端框架
wasm.sub.frontend.blurb = Leptos / Yew 用 Rust 写响应式前端。
wasm.sub.plugins.label = 插件系统
wasm.sub.plugins.blurb = 用 wasm 做安全、可热更新的插件 ABI。
wasm.crate.wasm-bindgen.blurb = Rust ↔ JS 互操作绑定生成
wasm.crate.wasmtime.blurb = Bytecode Alliance 的 wasm 运行时
wasm.crate.wasmer.blurb = 通用 wasm 运行时与包管理
wasm.crate.wit-bindgen.blurb = 组件模型 WIT 绑定生成
wasm.crate.leptos.blurb = 细粒度响应式的 Rust 前端框架
wasm.crate.trunk.blurb = Rust+WASM 前端打包工具

# ── CLI ──
cli.label = CLI
cli.tagline = 打造一流命令行工具：参数解析、TUI、进度反馈与分发。
cli.sub.args.label = 参数解析
cli.sub.args.blurb = 用 clap 的 derive 宏声明子命令、标志与校验。
cli.sub.tui.label = 终端 UI
cli.sub.tui.blurb = ratatui 构建全屏交互式终端界面。
cli.sub.output.label = 进度与输出
cli.sub.output.blurb = 进度条、彩色输出与人性化的状态反馈。
cli.sub.config.label = 配置
cli.sub.config.blurb = 分层配置：默认值、配置文件、环境变量、命令行。
cli.sub.testing.label = 测试
cli.sub.testing.blurb = 用 assert_cmd / trycmd 对 CLI 做端到端断言。
cli.sub.distribution.label = 分发
cli.sub.distribution.blurb = 交叉编译、静态链接与一键安装脚本。
cli.crate.clap.blurb = 功能完备的命令行参数解析器
cli.crate.ratatui.blurb = 构建终端用户界面（TUI）
cli.crate.indicatif.blurb = 进度条与 spinner
cli.crate.console.blurb = 终端样式、颜色与交互工具
cli.crate.crossterm.blurb = 跨平台终端操作库
cli.crate.assert_cmd.blurb = CLI 端到端测试断言

# ── 切换器（主题 / 语言） ──
theme.toggle = 切换主题
theme.heading = 主题
theme.reset = 重置为默认
lang.toggle = 切换语言
lang.heading = 语言
