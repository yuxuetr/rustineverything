# Cases 案例库规范
## 定位
Cases 是 Rust 项目案例库，不局限于前端站点。它覆盖 frontend、backend、fullstack、cli、embedded、ai、web3、library、tool、desktop 等项目类型，内容通过 git/PR 管理，文件系统是单一事实来源。
## 路由决策
SPA 页面使用 `/case` 和 `/case/:slug`。静态资源使用 `/cases/<slug>/...`，复刻课程模块“单数 SPA + 复数静态资源”的约定，避免 Axum `ServeDir` 与 Dioxus Router 冲突。
## 目录结构
每个案例放在 `assets/cases/<slug>/`。
- `case.yaml`：必填元数据。
- `README.md`：可选详情正文。
- `cover.webp`、`cover.jpg`、`cover.png`：可选封面，按 WebP、JPG、PNG 优先级选择。
## case.yaml Schema
必填或推荐字段：
- `name`：项目名。为空时从目录名生成。
- `slug`：可选。为空时使用目录名。
- `description`：卡片与搜索摘要。
- `category`：一级分类，取值为 `frontend|backend|fullstack|cli|embedded|ai|web3|library|tool|desktop`。
- `tags`：细粒度标签数组。
- `repo`：仓库 URL。
- `website`：可选网站 URL。
- `author`：作者或组织。
- `author_url`：可选作者链接。
- `language`：`rust|wasm|mixed`。
- `stars`：可选，MVP 阶段手填，缺省 0。
- `favorite`：可选，精选置顶。
- `date_added`：`YYYY-MM-DD`。
## 标签白名单
推荐标签：`axum`、`actix`、`dioxus`、`tauri`、`leptos`、`tokio`、`sea-orm`、`wasm`、`cli`、`embedded`、`web3`、`ai`、`fullstack`、`library`、`opensource`、`commercial`、`favorite`。
未知标签不会阻塞加载，会在 UI 中以“其他/待规范”样式显示，方便维护者后续整理。
## 规整规则
标签会 trim、转小写，并仅保留 ASCII 字母、数字、`-`、`_`。重复标签会去重。
未知 `category` 会归到 `tool`，未知 `language` 会归到 `rust`。
## 排序规则
案例列表排序为：`favorite` 优先，然后 `stars` 降序，然后 `date_added` 降序，最后 `name` 升序。
## 封面规范
优先提交 `cover.webp`，兼容 `cover.jpg` 和 `cover.png`。建议封面宽度不超过 1200px，文件不超过 200KB。未提供封面时前端渲染渐变占位，避免破图和额外请求。
## 搜索与论坛
Cases 会以 `kind="case"` 进入全站搜索，URL 为 `/case/<slug>`。详情页底部接入 `DiscussionPanel`，论坛引用使用 `ref_kind="case"` 和 `ref_path=<slug>`。
## 贡献流程
提交案例有两种方式：
1. 提交 GitHub Issue Form，提供项目元信息。
2. 直接发 PR，新增 `assets/cases/<slug>/case.yaml`，可选 `README.md` 和封面。
PR 中请确保 YAML 可解析、标签规整，并尽量提供简短 README 说明项目看点。
