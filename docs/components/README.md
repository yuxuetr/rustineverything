# MDX 嵌入组件目录

本目录每个 `.md` 文件对应一个可在 MDX 中使用的组件。所有组件遵循
[`MdxComponent` trait](../MDX_SPEC.md#5-编写新-mdx-组件30-行)：
`name() -> &'static str` + `render(attrs: &HashMap<String, String>) -> Element`。

## 内置组件（widgets crate 提供）

由 `rustineverything_widgets::register_default_components()` 在 app
启动期一次性注册。共 9 个：

### 视频嵌入

- [`<YouTube id="…" />`](./YouTube.md) — 16:9 YouTube 视频
- [`<Bilibili id="…" />`](./Bilibili.md) — 16:9 Bilibili 视频

### 文字高亮（Mac Preview 风格 5 色）

- [`<Yellow text="…" />`](./Yellow.md)
- [`<Green text="…" />`](./Green.md)
- [`<Blue text="…" />`](./Blue.md)
- [`<Pink text="…" />`](./Pink.md)
- [`<Purple text="…" />`](./Purple.md)

### 文本装饰

- [`<Underline text="…" />`](./Underline.md)
- [`<Strikethrough text="…" />`](./Strikethrough.md)

## 业务模块贡献的组件

由各 module crate 在自己的 `register_components()` 中注册。

- [`<PodcastCard id="…" />`](./PodcastCard.md) — `crates/modules/podcast`

## 写一个新组件

参考 [MDX_SPEC.md §5](../MDX_SPEC.md#5-编写新-mdx-组件30-行)。一般
30 行内即可完成 impl + register。
