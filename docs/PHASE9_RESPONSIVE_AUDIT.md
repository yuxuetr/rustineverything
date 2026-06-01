# Phase 9.4 — 响应式适配 Audit

> 个人博客 + 开源 fork 模式定位，读者主要在手机上看长文。
> 本文档记录 audit 扫描矩阵 / 发现的问题 / 修复前后对比。

## 扫描矩阵

| viewport | 尺寸 | 代表设备 |
|---|---|---|
| mobile | 375 × 667 | iPhone SE / 13 mini |
| tablet | 768 × 1024 | iPad mini portrait |
| desktop | 1280 × 800 | 主流笔记本 |

| 页面 | 路径 | 测试要点 |
|---|---|---|
| 首页 | `/` | navbar / hero / 最新文章列表 |
| Blog 列表 | `/blog` | 列表项排版 / 触摸目标尺寸 |
| Blog 详情 | `/blog/welcome` | 长文排版 / 代码块横向滚动 / 图片缩放 |
| Forum 列表 | `/topics` | 话题卡片 / 创建按钮 |
| Forum 详情 | （动态） | 回复树 / 输入框 |
| AI 板块 | `/ai` | 列表 + nav 抽屉 |
| Admin | `/admin` | 后台表格 / 表单 |

## 发现的问题

> 截图见 `docs/components/phase9-audit/`。

### P0 — 影响所有页面的 navbar

1. **站名 wrap 成 2 行**（mobile 375）：`"Rust in Everything"` 在窄屏分成 "Rust in" / "Everything"
   - **修**：Link 加 `whitespace-nowrap`；parent flex 加 `min-w-0`

2. **登录按钮文字 wrap**："登录" 显示成 "登\n录" 两行
   - **修**：button 加 `whitespace-nowrap`

3. **8 个板块 nav 在 mobile 上完全消失**：`hidden md:flex` 没 mobile 抽屉替代，窄屏用户没法跳到 AI / Web3 / 嵌入式 等板块
   - **修**：加 `md:hidden` hamburger button + 点击展开纵向 nav 抽屉

### P1 — 视觉细节

4. **Hero 标题对比度差**（home mobile）："专注 Rust 技术栈的学习与实战" 显示为浅灰，深背景下几乎看不见
   - 修复后：因 hero 用了响应式 text-flow 类，主题色生效后表现正常

### M0 — 工具链（修 navbar 时发现）

5. **Tailwind purge 没有 watch 模式**：项目用 Tailwind v4.1 但 Dioxus.toml `watch_path = ["src"]` + `ignore = ["assets"]`，dx serve 不会重 build tailwind.css。新增的类（如 `whitespace-nowrap`）必须手动跑 `npx -y @tailwindcss/cli -i crates/app/tailwind-input.css -o crates/app/assets/tailwind.css --minify`
   - **后续建议**：把这条加进 PLUGIN_DEV.md 或新增 `docs/DEV_WORKFLOW.md`，避免下次"加了 Tailwind 类但视觉无变化"被卡半小时

## 修复后（before / after 截图见 `docs/components/phase9-audit/`）

### Navbar（全站受益）

`crates/app/src/components/layouts/classic.rs`：

- 站名 Link 加 `whitespace-nowrap inline-block truncate max-w-32 sm:max-w-none`
  → mobile 截断为 "Rust in ..."，sm 以上完整显示
- 站名父 div 加 `min-w-0` → flex 子元素允许缩放
- 登录 button 加 `whitespace-nowrap`
- "开始学习" 同上
- 右侧 button group `gap-3` → `gap-2 sm:gap-3` → mobile 更紧凑
- 新增 `md:hidden` hamburger button：☰ / ✕ 图标互换
- 新增 mobile drawer：点 hamburger 展开纵向 nav，含 9 个板块 + "开始学习"
- 点链接后自动 `show_mobile_menu.set(false)` 收起

### 截图对比

| 页面 / viewport | before | after |
|---|---|---|
| home / 375 (mobile) | navbar wrap + 板块隐藏 + hero 标题低对比 | navbar 一行 + hamburger 抽屉 + hero 标题清晰 |
| home / 1280 (desktop) | OK | 无 regression |
| home / 375 + 抽屉展开 | — | 9 板块纵列显示，关闭按钮 ✕ |

### 未在本 phase 触碰（小型问题，留后续 phase）

- 评论区头像在 mobile 占比偏大（blog detail 末尾）
- Math 公式在 375 viewport 渲染过窄
- footer 链接字号在 mobile 偏小
- Blog list 卡片底部 tag 串 `RustDioxusTailwindTypography` 无分隔符

这些都是单文件改动，不阻塞 fork 用户在 mobile 上正常使用。

## Lighthouse Mobile（不强制；运维参考）

留待后续。Phase 9.4 范围内不强制跑 Lighthouse。
