# 项目架构评估总结报告

> 评估日期：2026-07-21 ｜ 治理落地：2026-07-21 ｜ 运行时验证：2026-07-24
> 范围：全仓（33 workspace crates + 独立 gateway workspace，约 31k 行 Rust）
> 方法：静态代码审查 + 依赖图分析 + 安全专项审计 + 全量测试/clippy 验证 + 真实环境烟测

## 1. 总体结论

本项目是一个**架构自觉性很高**的 Dioxus 0.7 全栈应用：单向依赖有明文规约并被执行
（`MODULE_SPEC.md` §11）、WASM 插件沙箱具备纵深防御、性能优化有注释级决策记录。
评估未发现需要推倒重来的结构性风险；识别出的 13 项风险集中在**会话生命周期、
密钥治理、运维可观测性**三类，均为增量改造，已在治理阶段（S1–S10）全部落地或
明确降级为可选项。

评分（治理后）：

| 维度 | 评估时 | 治理后 | 说明 |
| --- | --- | --- | --- |
| 架构分层 | 9/10 | 9/10 | 组合根 + IoC 模式成熟；main.rs 膨胀已拆分（S7） |
| 可扩展性 | 8/10 | 8/10 | 插件 ABI 版本治理完善；ABI v2 留待下个大版本 |
| 安全性 | 7.5/10 | 9/10 | S1/S2/S4/S5/S6/S8 落地后短板补齐 |
| 性能 | 7.5/10 | 8/10 | site.json mtime 缓存落地（S10）；i18n 热路径缓存为下一收益点 |
| 工程质量 | 8.5/10 | 9/10 | unwrap/expect lint 接入（S9）；693 测试 0 失败 |

## 2. 架构概览

```text
sdk-macros → sdk → {core, widgets}
core → llm
{core, widgets, sdk} → modules/* ×15   （单向：基础设施 → 业务模块）
modules/* → app                         （单向：业务模块 → 组合根）
plugins/*（WASM）--运行时加载--> core::PluginManager（wasmi 沙箱）
gateway（Pingora，独立 workspace）--反代--> app
```

核心设计要点：

- **组合根模式**：跨模块 UI 组合只发生在 `app`（Element 插槽注入）；跨模块数据
  依赖经 `core::engines::doc_source` IoC 注册表倒置。内容模块之间零横向依赖。
- **引擎层抽象**：`core/src/engines/` 下 10 个引擎（module/theme/auth/search/
  moderation/layout/content_transformer 等），插件按 capability 字符串路由。
- **插件沙箱四层防御**：fuel 上限、线性内存 cap、wall-clock 超时、输出长度
  clamp；外加 import 白名单（=∅）、SHA256 lock、manifest 一致性、CSS 反混淆扫描。
- **编译期分层**：`#[cfg(feature = "server")]` 严格切分 server-only 代码，
  客户端 WASM 不携带 sea-orm 等重依赖。

## 3. 风险登记与处置结果

| # | 风险 | 等级 | 处置 |
| --- | --- | --- | --- |
| R1 | JWT 7 天有效且无撤销机制 | 🔴 | ✅ S4：`users.token_version` + 写路径回查，角色变更即时吊销 |
| R2 | 加密密钥与 JWT_SECRET 同源、无轮换路径 | 🔴 | ✅ S5：独立 `DATA_ENCRYPTION_KEY` + `v2:` key-id 密文格式 |
| R3 | 迁移失败静默继续运行 | 🔴 | ✅ S3：`/healthz` degraded + `STRICT_MIGRATION=1` fail-fast |
| R4 | 应用层无速率限制 | 🟠 | ✅ S2：`/api/*` per-IP token-bucket（auth/pay 更严） |
| R5 | 主题 CSS 黑名单可被转义/空白混淆绕过 | 🟠 | ✅ S8：扫描前规范化（解码转义/去注释/去空白） |
| R6 | 安全响应头缺失 | 🟠 | ✅ S1：CSP/nosniff/Referrer-Policy/X-Frame-Options 中间件 |
| R7 | 支付回调缺并发防护与审计留痕 | 🟠 | ✅ S6：原子发货认领 + app_id/时间戳/mchid 校验 + pay_audit 日志 |
| R8 | main.rs 组合根膨胀、sitemap/feed 逻辑重复 | 🟠 | ✅ S7：拆分 4 个 server 子模块，条目收集统一 |
| R9 | i18n/主题 wasm 每渲染 instantiate | 🟡 | ⏳ 遗留可选：宿主侧翻译表缓存（下一个性能收益点） |
| R10 | PluginManager 全局 Mutex 缓存无上限 | 🟡 | ⏳ 遗留可选：RwLock/dashmap + 容量上限 |
| R11 | site.json 多点直读磁盘 | 🟡 | ✅ S10：`SiteConfig::load_cached` mtime 缓存（9 处热路径） |
| R12 | 生产路径残留 unwrap/expect | 🟡 | ✅ S9：workspace clippy lint（core/app/migration 接入） |
| R13 | 插件 ABI 精确版本匹配、演进成本高 | 🟡 | ⏳ 下个大版本议题（WIT/component model 评估） |

## 4. 治理落地记录（S1–S10）

| 任务 | 提交 | 内容 |
| --- | --- | --- |
| S1 | `baa7e56` | 全站安全响应头中间件（CSP 保守策略，`CSP_POLICY` 可覆盖） |
| S2 | `7159fd5` | per-IP token-bucket 限流（桶表有界 + 过期剪枝 + overflow 折叠） |
| S3 | `2bb1637` | `/healthz` ok/degraded + `STRICT_MIGRATION` fail-fast |
| S4 | `f6039a4` | JWT 即时吊销（token_version 迁移 + verified 会话 + admin bump） |
| S5 | `977e679` | 独立数据加密密钥 + `v2:` key-id 密文（v1 兼容解密保留） |
| S6 | `5197d7a` | 支付回调加固（原子认领/app_id 比对/时间戳新鲜度/审计日志） |
| S7 | `97bef3f` | router 组装拆分（seo/auth/pay/static 子模块） |
| S8 | `5a5617b` | CSS 扫描规范化反混淆 + 黑名单扩充 |
| S9 | `c5545b0` | clippy unwrap_used/expect_used lint（测试豁免，门禁 panic 标注） |
| S10 | `2117386` | `SiteConfig::load_cached` mtime 缓存 |
| 配置 | `ac13597` | compose/env 模板接入 S1–S5 运维开关 |

## 5. 验证结果

**静态验证（2026-07-21）**
- 全工作区 `cargo test --features server`：**693 通过 / 0 失败**
- `cargo clippy --workspace --features server --all-targets`：零警告
- server + 默认 web 双编译目标通过

**运行时烟测（2026-07-24，真实 PostgreSQL + server 二进制）**
- 迁移：`m20260721_000008_users_token_version` 成功应用；psql 确认列存在
- `/healthz`：`{"status":"ok","db":"connected","migrations":"applied"}`
- 安全头：四个响应头全部在线
- 限流：敏感接口连发 20 次 → 前 15 放行、第 16 起精确 429
- SSR：`/blog` 首屏含正文
- 独立密钥：注入 `DATA_ENCRYPTION_KEY` 后触发 PKCE 加密，无回退 warn

## 6. 遗留项与后续路线

**代码级可选增强（按性价比排序）**
1. i18n 翻译表宿主缓存（消除 per-render wasm instantiate，R9）
2. JWT exp 缩短（24h）或 refresh token；登出时 bump token_version 实现全设备下线
3. 首屏主题 CSS SSR 内联（消除 FOUC）
4. course 写路径升级 verified 会话；admin/moderation 的 site.json 读取接入 load_cached
5. PluginManager 缓存加容量上限 + Mutex→RwLock（R10）
6. 其余 crate 渐进接入 unwrap/expect lint；crypto v1 回退路径下版本移除

**需外部条件**
- 支付端到端验证（真实商户号 + 公网 HTTPS 回调）；对账定时任务与退款
- 浏览器 Console 确认无 CSP violation；gateway 反代下 `/healthz` 放行与 XFF 链
- CSP nonce 化（等 Dioxus 上游支持）；CSS 白名单解析器（lightningcss 成本评估）
- 插件 ABI v2（WIT/component model，下个大版本）

## 7. 相关文档

- 模块依赖规约：[`MODULE_SPEC.md`](MODULE_SPEC.md) §11
- 各子系统 SPEC：`docs/*_SPEC.md`
- 治理任务账本与验收记录：根目录 `Todos.md`「架构风险治理」章节
