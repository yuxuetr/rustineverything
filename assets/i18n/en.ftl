# English string dictionary (Option A: compile-time embedded, synchronous lookup in app_core).
# Format: key = value (# comments; dotted keys). Must mirror zh.ftl key set.

# ── Navigation ──
nav.blog = Blog
nav.podcast = Podcast
nav.forum = Forum
nav.cases = Cases
nav.start = Get Started
nav.embedded = Embedded
nav.course = Courses
nav.web3 = Web3
nav.wasm = WASM
nav.cli = CLI
nav.admin = 🛡️ Admin

# ── Dual-ecosystem nav (mega menus) ──
nav.eco.rust = Rust Ecosystem
nav.eco.ai = AI Ecosystem
nav.eco.rust.blurb = Industrial practice across embedded, Web3, WASM, CLI, and backend systems.
nav.eco.ai.blurb = LLMs, inference & deployment, agents, and the Rust AI ecosystem.
nav.ai.llm = LLMs
nav.ai.inference = Inference
nav.ai.agent = Agents
nav.ai.rust_ai = Rust AI
mega.col.domains = Domains
mega.col.learn = Learn
mega.learn.docs = Docs
mega.learn.courses = Courses
mega.learn.cases = Cases
mega.featured.cta = Browse featured cases

# ── Auth / User ──
auth.sign_in = Sign In
auth.sign_in_desc = Choose a method to continue
auth.continue_with = Continue with
auth.terms = By signing in, you agree to our Terms and Privacy Policy
auth.logout = Sign Out
user.my_topics = My Topics
user.my_annotations = My Annotations

# ── Blog ──
blog.title = Blog
blog.subtitle = Explore the boundless possibilities of Rust
blog.filter = Filter by Tag
blog.all = All
blog.empty = No articles yet
blog.articles = Articles
blog.no_results = No articles match this tag

# ── Footer ──
footer.tagline = Focused on the Rust stack

# ── Home Hero ──
hero.title = Learn and build with the Rust stack
hero.subtitle = Docs, blogs, courses, and cases in one place: AI, backend, frontend, cross-platform, Web3, Wasm, embedded, CLI, and more.
hero.btn.docs = Enter Docs
hero.btn.blog = Browse Blog
hero.btn.cases = View Cases
hero.btn.courses = View Courses
hero.stat.docs.value = Docs
hero.stat.docs.label = From zero to one
hero.stat.blog.value = Blog
hero.stat.blog.label = Continuously updated
hero.stat.course.value = Courses
hero.stat.course.label = Systematic learning
hero.stat.cases.value = Cases
hero.stat.cases.label = Reusable templates

# ── Home feature cards ──
home.section_title = Focused on the Rust ecosystem
home.section_subtitle = From low-level principles to full-stack practice — building high-performance, reliable software systems
home.browse.title = Browse by area
home.browse.subtitle = Dive into docs, cases, and courses across the Rust and AI ecosystems
home.card.basics.title = Rust Fundamentals
home.card.basics.desc = A clear take on ownership, lifetimes, traits, and other core concepts.
home.card.fullstack.title = Full-Stack Development
home.card.fullstack.desc = Build cross-platform apps fast with Dioxus, Axum, and SeaORM.
home.card.aiwasm.title = AI & WASM
home.card.aiwasm.desc = Explore high-performance WebAssembly compute and the Rust AI ecosystem.

# ── Home module grid cards (home.mod.<id>.title / .desc) ──
home.mod.docs.title = Docs
home.mod.docs.desc = From ownership to full-stack — a structured Rust learning path.
home.mod.blog.title = Blog
home.mod.blog.desc = Hands-on experience and technical notes, updated regularly.
home.mod.course.title = Courses
home.mod.course.desc = A progressive, systematic course series from basics to depth.
home.mod.cases.title = Cases
home.mod.cases.desc = Reusable engineering templates and real-world projects.
home.mod.ai.title = AI
home.mod.ai.desc = LLMs, inference, and the Rust AI ecosystem.
home.mod.wasm.title = WASM
home.mod.wasm.desc = High-performance WebAssembly compute, everywhere.
home.mod.web3.title = Web3
home.mod.web3.desc = Blockchain, smart contracts, and decentralized apps.
home.mod.embedded.title = Embedded
home.mod.embedded.desc = Drive MCUs and bare-metal development with Rust.
home.mod.cli.title = CLI
home.mod.cli.desc = Build fast, robust command-line tools.
home.mod.podcast.title = Podcast
home.mod.podcast.desc = Learn Rust by ear — progress even on your commute.
home.mod.forum.title = Forum
home.mod.forum.desc = Ask, discuss, and share — grow together with the community.

# ── Board shared UI ──
board.all = All
board.search = Search articles / tags…
board.featured = Featured crates
board.empty = No articles yet.
board.back_prefix = ← Back to
board.load_error_prefix = Failed to load: 

# ── Embedded ──
embedded.label = Embedded
embedded.tagline = Bare-metal and real-time systems in Rust: no_std, Embassy, RTIC, and mainstream MCU platforms.
embedded.sub.no-std.label = no_std
embedded.sub.no-std.blurb = Run Rust without the standard library on OS-less targets.
embedded.sub.embassy.label = Embassy
embedded.sub.embassy.blurb = An embedded async runtime — write interrupt-driven firmware with async/await.
embedded.sub.rtic.label = RTIC
embedded.sub.rtic.blurb = A hardware-priority concurrency framework with zero-cost task scheduling.
embedded.sub.hal.label = HAL / PAC
embedded.sub.hal.blurb = The embedded-hal abstraction and PAC crates — reuse drivers across chips.
embedded.sub.defmt.label = Logging & Debugging
embedded.sub.defmt.blurb = Efficient defmt logging plus a probe-rs flash/debug workflow.
embedded.sub.platforms.label = Platforms
embedded.sub.platforms.blurb = Hands-on with mainstream MCUs: RP2040, STM32, ESP32, nRF, and more.
embedded.crate.embassy.blurb = Embedded async runtime and HAL
embedded.crate.rtic.blurb = Real-time interrupt-driven concurrency framework
embedded.crate.embedded-hal.blurb = Cross-platform peripheral abstraction traits
embedded.crate.defmt.blurb = Efficient structured logging for embedded
embedded.crate.probe-rs.blurb = Flashing and debugging toolchain
embedded.crate.heapless.blurb = Heapless, fixed-capacity data structures

# ── AI ──
ai.label = AI
ai.tagline = Tensor compute, model inference, and LLM apps in Rust: candle, burn, and the ONNX ecosystem.
ai.sub.tensors.label = Tensor Compute
ai.sub.tensors.blurb = Tensor ops and autodiff on CPU, CUDA, and Metal.
ai.sub.inference.label = Inference
ai.sub.inference.blurb = Load pretrained weights for forward inference — deploy to server or edge.
ai.sub.llm.label = LLMs
ai.sub.llm.blurb = Run LLMs locally with quantization, KV cache, and streaming generation.
ai.sub.tokenizers.label = Tokenizers
ai.sub.tokenizers.blurb = BPE/WordPiece tokenization with HuggingFace tokenizers.
ai.sub.training.label = Training
ai.sub.training.blurb = Define networks, backprop, and optimizers in pure-Rust frameworks.
ai.sub.embeddings.label = Embeddings & Retrieval
ai.sub.embeddings.blurb = Sentence embeddings, similarity search, and vector DB integration.
ai.crate.candle.blurb = HuggingFace's minimalist tensor and inference framework
ai.crate.burn.blurb = Pure-Rust, multi-backend deep learning framework
ai.crate.tch.blurb = libtorch (PyTorch C++) bindings
ai.crate.tokenizers.blurb = HuggingFace's high-performance tokenizers
ai.crate.ort.blurb = Rust bindings for ONNX Runtime
ai.crate.safetensors.blurb = Safe, zero-copy tensor serialization format

# ── Web3 ──
web3.label = Web3
web3.tagline = Rust toolchains for blockchains and dApps: Ethereum, Solana, Substrate, and smart contracts.
web3.sub.evm.label = Ethereum / EVM
web3.sub.evm.blurb = Read/write on-chain state, send transactions, and parse event logs with alloy.
web3.sub.solana.label = Solana
web3.sub.solana.blurb = Solana program and client development for high-throughput on-chain logic.
web3.sub.substrate.label = Substrate
web3.sub.substrate.blurb = Build custom chains and runtime pallets with polkadot-sdk.
web3.sub.contracts.label = Smart Contracts
web3.sub.contracts.blurb = ink!, Solana programs, EVM bytecode, and contract interaction.
web3.sub.wallet.label = Wallets & Signing
web3.sub.wallet.blurb = Key management, signing schemes, and transaction construction.
web3.sub.indexing.label = On-chain Indexing
web3.sub.indexing.blurb = Block/event scanning, state reconstruction, and data services.
web3.crate.alloy.blurb = Modern Rust toolkit for Ethereum interop
web3.crate.revm.blurb = High-performance pure-Rust EVM implementation
web3.crate.solana-sdk.blurb = Solana on-chain program and client SDK
web3.crate.anchor.blurb = Solana program development framework
web3.crate.polkadot-sdk.blurb = Substrate / Polkadot blockchain framework
web3.crate.ink.blurb = Smart-contract eDSL for Substrate

# ── WASM ──
wasm.label = WASM
wasm.tagline = The WebAssembly landscape: browser interop, WASI, the component model, and server-side runtimes.
wasm.sub.bindgen.label = wasm-bindgen
wasm.sub.bindgen.blurb = Rust↔JS interop — compile Rust into the browser.
wasm.sub.wasi.label = WASI
wasm.sub.wasi.blurb = The WebAssembly System Interface: a portable ABI for files, clocks, and networking.
wasm.sub.components.label = Component Model
wasm.sub.components.blurb = WIT, wit-bindgen, and composable wasm components.
wasm.sub.runtimes.label = Runtimes
wasm.sub.runtimes.blurb = Embed wasm sandboxes on the server with wasmtime and wasmer.
wasm.sub.frontend.label = Frontend Frameworks
wasm.sub.frontend.blurb = Build reactive frontends in Rust with Leptos and Yew.
wasm.sub.plugins.label = Plugin Systems
wasm.sub.plugins.blurb = Use wasm for safe, hot-swappable plugin ABIs.
wasm.crate.wasm-bindgen.blurb = Rust↔JS interop binding generation
wasm.crate.wasmtime.blurb = The Bytecode Alliance's wasm runtime
wasm.crate.wasmer.blurb = General-purpose wasm runtime and package manager
wasm.crate.wit-bindgen.blurb = Component-model WIT binding generation
wasm.crate.leptos.blurb = Fine-grained reactive Rust frontend framework
wasm.crate.trunk.blurb = Build/bundle tool for Rust+WASM frontends

# ── CLI ──
cli.label = CLI
cli.tagline = Build first-class command-line tools: argument parsing, TUIs, progress feedback, and distribution.
cli.sub.args.label = Argument Parsing
cli.sub.args.blurb = Declare subcommands, flags, and validation with clap's derive macros.
cli.sub.tui.label = Terminal UI
cli.sub.tui.blurb = Build full-screen interactive terminal UIs with ratatui.
cli.sub.output.label = Progress & Output
cli.sub.output.blurb = Progress bars, colored output, and friendly status feedback.
cli.sub.config.label = Configuration
cli.sub.config.blurb = Layered config: defaults, files, environment variables, and CLI flags.
cli.sub.testing.label = Testing
cli.sub.testing.blurb = End-to-end CLI assertions with assert_cmd and trycmd.
cli.sub.distribution.label = Distribution
cli.sub.distribution.blurb = Cross-compilation, static linking, and one-line install scripts.
cli.crate.clap.blurb = Full-featured command-line argument parser
cli.crate.ratatui.blurb = Build terminal user interfaces (TUIs)
cli.crate.indicatif.blurb = Progress bars and spinners
cli.crate.console.blurb = Terminal styling, colors, and interaction utilities
cli.crate.crossterm.blurb = Cross-platform terminal manipulation library
cli.crate.assert_cmd.blurb = End-to-end CLI test assertions

# ── Pickers (theme / language) ──
theme.toggle = Switch theme
theme.heading = Theme
theme.reset = Reset to default
lang.toggle = Language
lang.heading = Language
