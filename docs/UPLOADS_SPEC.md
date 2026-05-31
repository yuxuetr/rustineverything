# Uploads 模块 SPEC

> 范围：`crates/modules/uploads` —— 用户图片上传，磁盘存储 + 多重校验 +
> 内容审核 hook。挂在评论 / 论坛 / 标注等富文本入口背后。

## 1. 设计选择

| 维度 | 选择 | 原因 |
| --- | --- | --- |
| 传输 | base64 字符串（HTTP POST JSON body） | 复用现有 Dioxus server fn 通道；不需要单独 multipart endpoint |
| 存储 | 磁盘 `assets/uploads/<filename>`（容器挂卷） | 简单、可备份、可前端 `/uploads/...` 直读；S3 / CDN 留作后续 |
| 文件名 | `<timestamp>_<sanitized-stem>_<rand6>.<ext>` | 避免冲突 + 避免原始文件名注入风险 |
| 大小限制 | **解码前 + 解码后双重 5 MB** | 解码前按 base64 长度估算（`len * 3/4`）提前拒绝，避免大缓冲分配 |
| 类型校验 | **magic bytes** 而非扩展名 | 扩展名可伪造；以 PNG/JPEG/GIF/WebP 头部魔数为准 |
| 鉴权 | server fn 内取 `current_session_user()`（可选） | 评论 / 论坛入口本身已强制登录；该 fn 直接被调用时 anonymous 仍允许（视调用方场景） |

## 2. 资产布局

```
assets/uploads/
├── 1717123456_my-photo_a1b2c3.png
├── 1717123589_screenshot_xyz789.jpg
└── …
```

文件直接由 axum `ServeDir` 在 `/uploads/*` 路径下静态服务（main.rs router）。
**docker-compose** 把 `app-uploads` 命名卷挂到容器 `/app/assets/uploads`，
跨重启 / 镜像升级保留用户图片。

## 3. server fn 契约

```rust
#[post("/api/upload")]
pub async fn upload_image(name: String, data_base64: String) -> Result<String, ServerFnError>;
```

入参：
- `name` —— 原始文件名（用于提取 stem，仅作显示 / debug；最终文件名由服务端重写）。
- `data_base64` —— base64 编码的图片字节，**接受**带 `data:image/png;base64,` 前缀
  的 data URL，也接受裸 base64。

返回：保存后的 URL，形如 `/uploads/1717123456_my-photo_a1b2c3.png`，前端直接
拼到 `<img src>`。

## 4. 处理流程（server）

按序执行；任何一步失败立即返回 `ServerFnError`：

1. **`check_upload_size(data_base64)`** —— 取末段 base64（剥 `data:` 前缀），
   按 `len * 3 / 4` 估算解码后字节数；> 5 MB 拒绝。**第一道防线**，避免分配
   超大解码缓冲。
2. **`base64::decode`** —— 实际解码。失败 → "解码失败"。
3. **解码后大小** —— `data.len() > 5 MB` 再拒一次（防御 base64 估算误差）。
4. **`sniff_image_mime(&data)`** —— 按 magic bytes 识别 MIME（PNG `89 50 4E 47`、
   JPEG `FF D8 FF`、GIF87a/89a、WebP RIFF/...WEBP）。其他 → "仅支持 png/jpg/gif/webp"。
5. **`safe_upload_filename(name, mime)`** —— 提取原文件 stem（仅保留 `[A-Za-z0-9_-]`
   前 40 字符；空则用 `"upload"`），按 MIME 选扩展名（png/jpg/gif/webp），拼
   `timestamp_stem_rand6.ext`。
6. **扩展名白名单**（双保险）—— 最终 filename 的扩展名必须在 `["png","jpg","jpeg","gif","webp"]`。
7. **ModerationEngine 视觉审核**（Phase 4）——
   - 调 `module_moderation::evaluate_submission`，把图片以 base64 data URL
     形式传给 vision LLM（**不**把私有图暴露到外部 URL）。
   - `Block` → 拒绝 + warn 日志（user / filename / score / reason）；**不写盘**。
   - `Flag` → 写盘 + 入审核队列（管理员后台批量处理）。
   - `Allow` → 直接写盘。
   - 审核默认关闭（`site.json::moderation.enabled = false`）→ 直接 Allow，零开销。
8. **写盘** —— `fs::write(assets/uploads/<filename>, data)`。
9. **返回** `format!("/uploads/{}", filename)`。

## 5. 安全考量

| 攻击面 | 防御 |
| --- | --- |
| 超大上传打满磁盘 | 双重 5 MB 限制（base64 估算 + 解码后字节） |
| 路径穿越（`../../etc/passwd`） | `safe_upload_filename` 只取 stem 字符 `[A-Za-z0-9_-]`，不接受 `/` `\` `.` |
| 扩展名伪造（`.exe.png`） | magic bytes 校验 + 扩展名白名单双保险 |
| 内容违规（NSFW / 暴力 / 政治） | ModerationEngine vision LLM hook，Block 不写盘 |
| 私有图泄露到第三方 | 审核走 base64 data URL，不传公网可访问 URL |
| 文件名冲突 / 覆盖 | timestamp + 6 字符随机后缀 |

## 6. ModuleEngine 集成

`site.json::modules.uploads.enabled` 当前**未实现 server-side gate**——上传
fn 默认可调用。前端组件应在不需要上传场景隐藏入口。完全 gate 留待后续。

## 7. 测试覆盖

```bash
cargo test --features server -p module-uploads
```

**12 个单测**，覆盖：

- `check_upload_size`：边界（恰好 5 MB / 5 MB+1 / 空 / 有 data: 前缀）。
- `sniff_image_mime`：PNG / JPEG / GIF87a / GIF89a / WebP 各识别；未知字节 → None。
- `safe_upload_filename`：原文件名带 `..` / `/` / 中文 / 超长 → 全部安全化；
  时间戳 + 随机后缀去重。
- 不支持的 MIME → Err。

未覆盖（依赖外部资源）：
- 实际写盘（需 tempdir，未做）。
- ModerationEngine vision 调用（live LLM 测试 ignored，按需 `--ignored` 跑）。

## 8. 性能 / 容量

- 单图 ≤ 5 MB，base64 解码占用 ≈ 7 MB 短时内存，可接受。
- 磁盘容量按 `app-uploads` 卷规划；docker-compose 默认本地卷，长期需挂 NFS / S3 副本。
- 上传 QPS 期望 < 10/s；高并发场景可在 Pingora / 反向代理层加 rate limit。

## 9. 不在本期范围

- 视频 / GIF 动图大文件（>5 MB）
- S3 / R2 / OSS 等对象存储 backend
- 图片自动压缩 / 缩略图生成（前端可在上传前压缩）
- EXIF 隐私字段清除（建议前端预处理）
- 多文件批量上传（当前一次一张）
- CDN 集成（当前直接 `ServeDir` 静态服务）
