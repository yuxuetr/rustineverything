# OAuth 授权模块设计规范 (Auth Architecture Spec)

认证内置在宿主（R1 起不再有 WASM 认证插件）。流程、provider 端点与字段映射、
环境变量、新增 provider 的步骤见 [`AUTH_GUIDE.md`](AUTH_GUIDE.md)；代码在
`crates/core/src/auth/`（`provider.rs` 为四个内置 provider 的 `ProviderSpec` 与 `map_profile`）。

本文只保留安全策略与权限分级。

---

## 1. 安全策略 (Security)

1.  **端点固定**：授权 / token / 用户信息端点是代码里的 https 常量，不来自配置或用户输入。
2.  **Secret 只在服务端**：`Client Secret` 只从环境变量读取，不出现在 site.json、日志或客户端。
3.  **CSRF / PKCE**：服务端生成 `state`（X 另加 PKCE S256 verifier），加密写入短时 cookie，回调时强制校验。
4.  **用户 id 必须可得**：`map_profile` 取不到稳定的第三方用户 id 时登录失败，不落到默认 id。
5.  **生产环境回调**：确保在各平台后台注册的回调地址为 `https://rustineverything.app/api/auth/callback/{provider}`。

---

## 2. 权限控制 (Access Control)

系统将权限分为三级：
*   **Public**: 匿名可见（博客列表、文章详情、播客列表）。
*   **Member**: 登录可见（评论发表、收藏、案例实战课）。
*   **Admin**: 管理员可见（用户角色、内容删除、审核队列与审核设置）。
