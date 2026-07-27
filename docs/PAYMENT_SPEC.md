# 支付系统规范 (M5)：微信支付 + 支付宝（国内）

> 本文是 M5 在线支付的落地设计。建立在 M4 付费地基之上：课程 `access_tier/price`、
> `entitlements` 表、`get_lesson` 服务端鉴权、Paywall 已就绪（见
> [SITE_REDESIGN_SPEC](./SITE_REDESIGN_SPEC.md) §5、[COURSE_SPEC](./COURSE_SPEC.md)）。
> M5 只新增「下单 → 支付 → 回调 → 写权益」这条链路。

## 1. 定位与范围
- **网关**：支付宝（电脑网站 / 手机网站 / 扫码）+ 微信支付 v3（Native 扫码 / H5）。
- **商品**：一次性购买单门课程（`orders.course_slug`）。订阅会员留 [M6]。
- **不在范围**：发票开具、优惠券、分账、跨境（国内主体收款，税务/开票自理）。
- **前置条件（业务侧）**：支付宝开放平台应用 + 网页支付能力；微信支付商户号（v3）。
  两者均需**企业资质**与备案域名；本规范假定已具备。

## 2. 数据模型

新增 `orders` 表（SeaORM 实体 + 迁移，复刻 M4 entitlements 套路）：

```
orders(
  id            bigserial PK,
  out_trade_no  varchar(64) UNIQUE,   -- 我方订单号（下单时生成，传给网关）
  user_id       int  FK users(id) ON DELETE CASCADE,
  course_slug   varchar(128),
  provider      varchar(16),          -- wechat | alipay
  scene         varchar(16),          -- native | h5 | page | wap | qr
  amount        bigint,               -- 分；下单时从 course.price 快照，回调时核验
  currency      varchar(8) default 'CNY',
  status        varchar(16) default 'pending',  -- pending|paid|failed|closed|refunded
  provider_txn  varchar(64) null,     -- 网关流水号（transaction_id / trade_no）
  created_at    timestamptz default now(),
  paid_at       timestamptz null
)
索引：idx_orders_user(user_id)、idx_orders_status(status, created_at desc)
```

**与 entitlements 的关系**：订单 `paid` 时，幂等写入 `entitlements(user_id, course_slug,
source='purchase')`（复用 M4 的 grant 逻辑）。`get_lesson` 鉴权天然解锁，无需改动。

## 3. 整体流程（时序）

```
用户在 Paywall 点「购买」
  → [client] create_order(course_slug, provider, scene)        server fn（需登录）
       · 校验：课程 paid 且未拥有；价格快照
       · 建 order(pending, out_trade_no)
       · 调网关「统一下单」→ 拿 支付凭据
       ← 返回 { kind, payload }：
            page/wap → 跳转 URL（自动提交表单 / 302）
            native   → code_url（前端渲染二维码）
            h5       → h5_url（移动端跳转）
  → 用户完成支付
  → [网关] 异步回调 notify_url（Axum 原生路由，非 server fn）
       · 验签（+ 微信 v3 AES-GCM 解密）
       · 核验金额 == order.amount、状态成功
       · 幂等：order 已 paid 则直接回 200/success
       · 否则：order→paid + 写 entitlement(source=purchase)
       · 按网关格式回应（微信 {code:SUCCESS} / 支付宝 "success"）
  → [client] 扫码场景轮询 query_order(out_trade_no)；跳转场景靠 return_url
       · status=paid → 刷新 list_my_entitlements → 解锁/跳转课节
```

## 4. 支付宝集成

- **接口**
  - 电脑网站：`alipay.trade.page.pay`（返回自动提交的 HTML 表单 → 跳转收银台）。
  - 手机网站：`alipay.trade.wap.pay`。
  - 扫码（当面付）：`alipay.trade.precreate` → `qr_code`；前端渲染二维码 + 轮询 `alipay.trade.query`。
- **签名**：请求用我方应用私钥 RSA2 签名；回调/同步返回用**支付宝公钥**验签。
- **回调**：`notify_url` 收 form-urlencoded，验签 + `trade_status ∈ {TRADE_SUCCESS, TRADE_FINISHED}`，
  核 `out_trade_no` / `total_amount` / `app_id` / `seller_id`，幂等后回纯文本 `success`。
- **同步返回**：`return_url` 仅用于前端体感跳转，**不作为发货依据**（以异步 notify 为准）。

## 5. 微信支付 v3 集成

- **接口**
  - Native（PC 扫码）：`POST /v3/pay/transactions/native` → `code_url`（渲染二维码）。
  - H5（移动浏览器）：`POST /v3/pay/transactions/h5` → `h5_url`。
  - 查询：`GET /v3/pay/transactions/out-trade-no/{out_trade_no}?mchid=...`。
  - （JSAPI/小程序需 openid，不在本期。）
- **鉴权**：请求头 `Authorization: WECHATPAY2-SHA256-RSA2048 ...`，用**商户 API 私钥**签名
  （method+url+timestamp+nonce+body）。
- **回调验签 + 解密**：`notify_url` 收 JSON；用 `Wechatpay-Signature/-Timestamp/-Nonce/-Serial`
  头 + **平台证书**验签；再用 `APIv3Key` 对 `resource`（AEAD_AES_256_GCM）解密得明文，
  核 `out_trade_no` / `amount.total` / `trade_state=SUCCESS`，幂等后回 `{"code":"SUCCESS"}`。
- **平台证书**：启动期或定时从 `GET /v3/certificates` 拉取并缓存（用于验签），随轮换刷新。

## 6. 服务端接口

**Dioxus server fn（业务、走 cookie 鉴权）**
- `create_order(course_slug, provider, scene) -> OrderInit`：登录校验 + 建单 + 调网关下单。
- `query_order(out_trade_no) -> OrderStatus`：扫码场景前端轮询（也可主动查网关回填）。
- `list_my_orders() -> Vec<OrderInfo>`：我的订单（个人中心，可选）。
- Admin：`list_orders` / `close_order` / `refund_order`（可选，M5e）。

**Axum 原生路由（网关回调，加在 `main.rs::dioxus::serve` 的 router 定制处，与 ServeDir 并列）**
- `POST /api/pay/alipay/notify`
- `POST /api/pay/wechat/notify`
- 回调处理器**不依赖 Dioxus 上下文**：直接读 body + 头，验签，更新 DB（共享连接池），
  写 entitlement，返回网关要求的响应体。

## 7. 客户端 UX
- Paywall（M4c）/ 课程详情加「购买 ¥{价}」按钮 → 打开 **PurchaseModal**：
  - 选 支付宝 / 微信；PC 默认 扫码/page，移动端 H5/wap（按 UA 或让用户选）。
  - `page/wap/h5`：拿到 URL 后跳转（支付宝 page 用自动提交表单）。
  - `native`：弹二维码（`code_url` → 前端二维码库渲染）；每 ~2s 轮询 `query_order`。
- 支付成功（轮询到 paid 或 return_url 回站）→ 刷新权益 → 关闭 Paywall / 跳到课节。
- 失败 / 超时（如 5 分钟）→ 提示重试；订单置 `closed`。

## 8. 配置与密钥（`.env`，启动期 `assert_not_placeholder` 校验，不回显）
```
# 支付宝
ALIPAY_APP_ID=
ALIPAY_APP_PRIVATE_KEY=        # 我方应用私钥（PEM）
ALIPAY_PUBLIC_KEY=             # 支付宝公钥（验签）
ALIPAY_GATEWAY=https://openapi.alipay.com/gateway.do
# 微信支付 v3
WECHAT_MCHID=
WECHAT_APP_ID=                 # 绑定的 appid（Native/H5 需要）
WECHAT_API_V3_KEY=            # APIv3 密钥（回调解密）
WECHAT_MCH_PRIVATE_KEY=       # 商户 API 私钥（PEM）
WECHAT_MCH_SERIAL_NO=         # 商户证书序列号
# 公共
PAY_NOTIFY_BASE=https://<公网域名>   # 拼 notify/return URL；必须 HTTPS
```

## 9. 幂等 / 安全 / 对账
- **验签是发货前提**：任何未通过验签的回调一律拒绝，绝不据此发货。
- **金额核验**：回调金额必须 == `order.amount`，币种 CNY；不符记异常、不发货。
- **幂等**：以 `out_trade_no` 为键；`order.status==paid` 直接成功返回；entitlement 用
  `ON CONFLICT DO NOTHING/UPDATE`（M4 已是幂等）。
- **回调可重复**：网关会重试直到收到成功响应 → 处理器必须可重入。
- **对账（M5e，可选）**：定时任务扫 `pending` 超 N 分钟的单，主动查网关回填/关单。
- **不记密钥日志**；HTTPS-only notify；out_trade_no 不可猜（含随机段）。

## 10. Rust 依赖选型（生态优先，见 [[feedback_rust_ecosystem_first]]）
- 候选：`wechat-pay-rust-sdk`（v3 Native/H5 + 回调解密）、`alipay-rs` / `alipay-sdk-rust`。
- 选型前需核：维护活跃度、v3 平台证书轮换支持、AES-GCM 解密正确性。
- 兜底：用 `rsa` + `sha2` + `base64` + `aes-gcm` 自行实现签名/验签/解密（v3 回调需 AES-256-GCM）。
- 生产 HTTP 客户端正常走公网，无需测试用的 `.no_proxy()`。

## 11. 本地开发与测试
- **沙箱**：支付宝有沙箱（alipaydev）；微信 v3 用真实商户小额自测（沙箱能力有限）。
- **公网回调**：本地需隧道（frp / ngrok / cloudflared）把 `PAY_NOTIFY_BASE` 指到本机
  `dx serve`（参考 [[project_run_local]] localhost:8080）。
- **回调单测**：对验签 / 解密 / 金额核验 / 幂等写权益做单元测试（构造签名样本），
  无需真实网络。
- **手动验证**：沙箱下单 → 完成支付 → 确认 order=paid、entitlement 写入、课节解锁。

## 12. 分阶段（建议 Todos）
- **M5a**：`orders` 实体 + 迁移；`create_order` / `query_order` server fn（先打通建单 +
  状态机，网关调用可先 stub / 沙箱）。
- **M5b**：支付宝接入（page/wap/precreate + `/api/pay/alipay/notify` 验签发货）。
- **M5c**：微信支付 v3 接入（native/h5 + `/api/pay/wechat/notify` 验签+解密发货 + 平台证书）。
- **M5d**：PurchaseModal（选网关 + 二维码/跳转 + 轮询解锁）；接到 Paywall / 课程详情。
- **M5e（可选）**：我的订单页、对账定时任务、退款。

## 13. 风险与前置条件
- **资质**：两网关均需企业商户号 + 备案 HTTPS 域名；个人主体无法开通 → 业务前置。
- **微信 v3 复杂度**：平台证书轮换 + 回调 AES-GCM 解密是最易错处，务必单测覆盖。
- **回调可达性**：notify URL 必须公网可达且 HTTPS；反代（Pingora/nginx）需放行 `/api/pay/*`。
- **退款/客诉**：本期仅手动（admin + 网关后台）；自动退款留 M5e。
