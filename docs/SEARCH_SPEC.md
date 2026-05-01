# 搜索功能规范 (3.2)

## 1. 选型决策
经横向对比 PostgreSQL FTS、Tantivy embedded、Meilisearch:

- **方案**:Tantivy embedded(`tantivy 0.26` + `tantivy-jieba 0.19`)
- **理由**:全 Rust 栈天然契合;BM25 打分质量高于 PG FTS;无需额外部署独立服务;通过 `tantivy-jieba` 中文分词体验良好;面向"个人/小团队站"的运维成本最低。
- **保留逃生通道**:server fn 接口形态(`/api/search/query` + `SearchHit`)与具体引擎无关,后续若切 Meilisearch 只需替换 `engine.rs`,前端零改动。

## 2. 模块拆分
新建 `crates/modules/search`(与 forum/admin 风格一致):
- `text.rs` —— 纯逻辑文本工具(`strip_frontmatter` / `markdown_to_plain` / `truncate_chars`),前后端共享。
- `indexer.rs` —— 索引源汇总:扫描 `assets/posts/` 与 `assets/docs/`,从 PostgreSQL 拉取 `topics`。
- `engine.rs` —— 基于 tantivy 的 RAM 索引:schema、jieba 分词器注册、查询接口、全局单例 + lazy 构建。
- `server.rs` —— `search_query` / `search_reindex` 两个 server fn。
- `search.rs` —— Dioxus 前端:`SearchButton` 导航栏入口 + `SearchModal` 全屏检索面板 + Cmd+K 快捷键。

## 3. Schema
| 字段 | 类型 | 用途 |
|---|---|---|
| `kind` | STRING + STORED + FAST | 过滤(blog/doc/topic) |
| `ref_id` | STRING + STORED | 资源 id(slug/path/topic_id) |
| `title` | TEXT(jieba) + STORED | 标题 |
| `body` | TEXT(jieba) + STORED | 正文 |
| `url` | STRING + STORED | 跳转 URL |
| `created_at` | STRING + STORED | 显示日期 |

`title` 字段在查询时 boost = 3.0,标题命中权重显著高于正文。

## 4. 索引源(本期 MVP)
| kind | 来源 | id |
|---|---|---|
| `blog` | `assets/posts/<slug>/index.{md,mdx}` | slug |
| `doc` | `assets/docs/<path>/index.{md,mdx}`(递归,跳过 `_xxx`/`.xxx`) | 相对路径 |
| `topic` | DB `topics`(全表) | topic id |

未在本期纳入:课程 lesson、评论、标注。后续追加时仅扩展 `indexer.rs::collect_documents`,无需触动 schema。

## 5. API 契约
### `POST /api/search/query`
入参:
- `q: String` —— 查询文本;空白返回空结果。
- `kind: Option<String>` —— `blog`/`doc`/`topic`;非法值忽略。
- `limit: Option<u32>` —— 默认 20,上限 50。

返回 `SearchResponse`:
```text
{
  hits: [{ kind, ref_id, title, snippet, url, created_at, score }],
  total: usize,
  elapsed_ms: u64,
}
```

### `POST /api/search/reindex`
要求 admin。强制重建索引;返回中文成功消息。

## 6. 前端交互
- 导航栏右上角放大镜按钮 `SearchButton` —— 显示"⌘K"提示。
- 全局键盘监听:`Cmd/Ctrl+K` 切换面板;`Esc` 关闭。
- 200ms 输入防抖,防止打字过程过频请求。
- `SearchModal` 在 App 根挂一次(`use_search_open_provider`),所有页面共享。
- 结果按 kind 显示彩色徽章(BLOG/DOC/TOPIC),命中片段最长 200 字符,标题 + URL + 日期 + 分数。

## 7. 索引生命周期
- **Lazy 构建**:首次调用 `search_query` 时触发 `engine::get_or_build`,扫描资产目录 + DB,RAM 写入索引。
- **强制重建**:管理员调 `search_reindex`(后续可在 `/admin/plugins` 旁追加按钮),`engine::rebuild` 替换全局单例。
- **进程重启**:RAM 索引随进程消失,下次首查自动重建。当前单实例部署可接受;未来切磁盘 `MmapDirectory` 时只改 `engine::SearchEngine::build_with_documents`。

## 8. 中文分词
- `tantivy-jieba::JiebaTokenizer::default()` 注册为名为 `jieba` 的 tokenizer。
- title/body 字段在 schema 中固定使用该分词器。
- 不依赖 PG `zhparser`,无需额外服务端扩展。

## 9. 安全/防御
- 查询字符串先经过 `escape_query`(剥离 `+ - ! ( ) [ ] ^ ~ * ? : \ /`),再丢给 QueryParser;若仍解析失败,降级为 `parser_safe`(仅保留字母数字与非 ASCII 字符)。
- 数据库不可用时索引仅含文件系统内容,不阻塞查询。
- `reindex` 接口由 `core::session::require_admin` 保护。

## 10. 测试覆盖
`cargo test --features server -p rustineverything-module-search` 共 34 个单元测试:
- `text`:frontmatter / 标题 / 链接 / 图片 / 中文 / 截断(11)
- `indexer`:frontmatter kv / md 解析 / 递归扫描 / 跳过隐藏目录(5)
- `engine`:schema 字段 / 转义 / snippet / 实际查询(英文/中文/kind 过滤/特殊字符/空查询)(13)
- `server`:`clamp_limit` / `normalize_kind` 范围与边界(5)

## 11. 不在本期范围
- 拼写容错(typo tolerance)
- 索引磁盘持久化(MmapDirectory)
- 评论/标注/课程 lesson 索引源
- 高亮 HTML(目前 snippet 是纯文本)
- 同义词字典 / 关键词高亮的服务端 hl 标签
- `scripts/reindex.sh` 通过 admin 接口触发(后续可加 curl 命令封装)
