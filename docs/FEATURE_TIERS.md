# 功能分层（Cargo features）设计

2026-10-20。目标：同一套代码按编译选项产出四种站点，从「普通博客」到「博客 + 论坛 + 课程售卖 + 打赏」。

## 1. 四层与 feature

| 层 | feature | 包含 | 运行时依赖 |
| --- | --- | --- | --- |
| basic | （无，始终编译） | 博客、文档、播客、案例、5 个专题板块、搜索、About、主题 / 明暗 / 中英、SEO（sitemap / feed） | 无数据库、无 `JWT_SECRET` |
| 账号 | `accounts` | 登录（OAuth）、论坛、博客评论、图片上传、内容审核、管理后台（用户 / 评论 / 话题 / 审核） | Postgres、`JWT_SECRET` |
| 课程 | `course` = `accounts` + 课程 | 课程 / 课时 / 付费墙、标注、学习进度、权益（admin 手动开通）、Pro 会员 | 同上 |
| 在线支付 | `payments` = `course` + 网关 | 微信 / 支付宝下单、回调发货、对账、我的订单 | 同上 + 商户密钥 |
| 打赏 | `tips` = `accounts` + 网关 | 见 §6，**待作者确认后开工** | 同上 + 商户密钥 |

`default = ["web", "payments"]`，即今天的完整站点；不传 feature 的构建行为与现在一致。

```toml
# crates/app/Cargo.toml
accounts = ["dep:module-forum", "dep:module-comments", "dep:module-admin", "dep:module-uploads", "dep:migration"]
course   = ["accounts", "dep:module-course"]
payments = ["course", "module-course/payments"]
server   = [..., "module-forum?/server", "module-course?/server", ...]   # 弱依赖：开了层才级联
```

基础层用 `cargo build -p app --no-default-features --features web`（server 侧同理换 `server`）。dx 用同一组 feature 构建 web 与 server 两端（见 §5「两端 feature 必须一致」）。

## 2. 现状（证据）

- 每个功能已是独立 crate；Dioxus server fn 随 crate 链接注册——**不链接 crate，接口就不存在**，不用另写开关。
- 已有先例：`payments` feature（PM4）关闭时不注册 `/api/pay/*`，CI 有 `--no-default-features` 检查。
- 已有运行时开关：`site.json::modules.<id>.enabled` → `ModuleEngine` → 导航 `on_forum` / `ModuleGate` / sitemap / 搜索过滤。
- 内容模块（blog / podcast / docs / cases / board）不碰数据库（`grep -rlE 'get_or_init_pool|entities::|sea_orm'` 在这些 crate 无结果）；搜索只在收录论坛话题时读库。
- 跨层耦合（全部在 app 组合根或可移到组合根）：
  - `module-cases` 直接依赖 `module-forum`（案例详情嵌 `DiscussionPanel`）；
  - `module-cases`、`module-course` 声明依赖 `module-blog`，代码里没有任何引用；
  - 博客 / 文档页嵌入课程 crate 的 `AnnotationLayer` 与论坛的 `DiscussionPanel`（docs 已用 `footer` 插槽从 app 注入）；
  - 首页、导航、页脚、用户菜单、hero 等约 60 处引用论坛 / 课程 / 登录。

## 3. 关键设计决定

### DL1 路由枚举不按 feature 删变体；未编译的页面渲染 404

实验（2026-10-20）：给 `Route::Podcast` 加 `#[cfg(feature = "x")]`，`Routable` derive 正常，错误只出现在引用处。但引用处在 `rsx!` 里只能写 `if cfg!(...)`——`cfg!` 不移除代码，变体仍须存在。删变体意味着每个链接都得拆出 rsx 外用 `#[cfg]` 包，约 60 处。

所以：`Route` 不变；各层页面组件移到 `routes/<层>.rs`，未开层时由 `absent_pages!` 宏生成同名组件，渲染 `widgets::NotFound`（SSR 回 404）。导航 / 页脚 / 首页不用逐个 `cfg`——它们本来就按「启用模块」列表显示（DL2）。

### DL2 「编译进来的模块」进入 ModuleEngine，运行时开关只能在这个集合里再关

`app_core::engines::module::register_compiled_modules(&[...])`：app 在 `main` 开头（client 与 server 两侧）登记本次编译带进来的模块 id；`default_module_specs()` 只返回登记过的（未登记 = 全部，单测与旧行为不变）。于是导航、`ModuleGate`、sitemap、搜索的过滤全部自动跟随编译选项。与 `register_doc_source` 同一种组合根注入方式。

不属于「模块」的登录入口（登录按钮、用户菜单、登录弹窗、`get_current_user`）在 app 里按 `cfg!(feature = "accounts")` 处理，约 6 处。

### DL3 标注归课程层（当前版本不搬代码）

标注实现在 `module-course`，博客 / 文档也在用。只开 `accounts` 时博客 / 文档没有标注。不把标注拆成独立 crate：目前没有「要标注、不要课程」的部署。
重估条件：出现只开 `accounts` 不开 `course` 的部署配置——`grep -rhoE 'FEATURES=[^ ]*' Dockerfile .github docker-compose*.yml | grep accounts | grep -v course` 有输出。

### DL4 数据库迁移不拆

低层用不到的表空着无害；拆迁移会打乱已部署环境的迁移历史。basic 层不连库，不跑迁移。

### DL5 app-core 当前不拆 feature

basic 层仍编译 core 的 sea-orm / oauth2 / jsonwebtoken，但运行时不需要数据库和 `JWT_SECRET`。拆 core 的收益只有构建体积与时间，先在 L7 测数再定。

## 4. 任务（Todos 阶段 L）

1. **L1 去掉无用 / 反向依赖**：删 cases、course 对 blog 的依赖；案例详情的 `DiscussionPanel` 改由 app 经 `footer` 插槽注入（与 docs 同）。验收：`cargo tree -p module-cases` 无 forum / blog；案例详情 SSR HTML 与改前一致。
2. **L2 编译模块登记**：DL2 的 `register_compiled_modules`；hero 与生态菜单里的课程 / 案例 / 文档链接改为按启用列表显示（现在 site.json 关掉课程后这两处仍有链接，是已有 bug）。单测：登记后 specs 只含登记项。
3. **L3 `course` 层**（先于 accounts：课程依赖登录，反过来切会出现「有课程无登录」的组合）：课程改可选依赖；课程页、标注、我的标注 / 订单、权益后台、`/courses` 静态鉴权、sitemap 课程项、课程进搜索、首页课程区按 feature。验收：`--features server,accounts` 编译并运行，`/course` 404、博客页无标注；完整构建不变。
4. **L4 `accounts` 层**：论坛 / 评论 / 上传 / 审核 / 管理后台改可选依赖；页面拆到 `routes/accounts.rs` + 404 桩；登录入口按 feature；启动时只在 `accounts` 下校验 `JWT_SECRET`、连库、迁移，挂 auth 路由。验收：basic 两端编译通过；basic 服务端不配 `DATABASE_URL` / `JWT_SECRET` 能启动，`/`、`/blog`、`/docs`、`/podcast`、`/case` 200，`/topics`、`/admin` 404，页面无登录按钮；完整构建行为不变（`scripts/check_page_titles.sh` 通过）。
5. **L5 About 页**（basic）：`/about` 渲染 `assets/pages/about.md`（frontmatter 给 title / description），文件不存在时 404；页脚链接按文件是否存在显示。
6. **L6 分层门禁**：`scripts/check_tiers.sh` 对 basic / accounts / course / 完整四档各跑 server 与 web 两次 `cargo check`；CI 替换现有「no payments」两步；Dockerfile 加 `ARG FEATURES`；DEVELOPER / DEPLOY 文档写明。反向验证：在 basic 可达的代码里加一处对 `module_forum` 的无门控引用，脚本必须失败。
7. **L7 量一下 basic 与完整版的差距**：release 服务端二进制大小、依赖数、冷构建时间；据此决定 DL5 是否拆 core，结论写回本文档。

## 5. 风险

- **两端 feature 必须一致**：`Route` 不变，所以两端路由表天然一致；但页面组件的实现取决于 feature，web 端与 server 端用了不同 feature 会导致 hydration 不匹配。dx 对两端传同一组 `--features`，手写构建命令时要注意。
- **层腐化**：不常编译的组合很快会编译不过——L6 的门禁就是为此。
- **`cargo test --workspace` 会合并 feature**：工作区测试始终是完整版；各层只靠 `check_tiers.sh` 的编译检查与 L3/L4 的手工运行验收。

## 6. 打赏（待作者确认 DL6 后开工）

推荐形态（需确认）：
- 必须登录（复用 `orders.user_id NOT NULL`、我的订单、对账任务）；
- 固定档位（如 ¥5 / 10 / 20 / 50），不收留言；打赏对象是站点本身，不分作者；
- 入口：博客文章末尾、播客页；
- 数据：`orders` 加 `product_kind`（`course` | `tip`，默认 `course`）与通用的 `product_ref`，支付流水线 `on_paid` 按 kind 分派（tip 不发权益）——这一步同时是「会员自助购买」（M6 余项）的前置。

任务：T1 订单加 product_kind 迁移 + 流水线分派；T2 下单 server fn（`tips` feature）；T3 打赏按钮 + 我的订单显示打赏；T4 管理后台打赏列表。
