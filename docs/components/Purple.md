# `<Purple />`

紫色高亮文字（Mac Preview 标注色 `#A855F7`）。

## 用法

```mdx
<Purple text="这段是紫色高亮" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要高亮的文字内容 |

## 输出

```html
<span style="color: #A855F7; font-weight: 600">这段是紫色高亮</span>
```

## 代码

`crates/widgets/src/components.rs::ColorComponent`（`name = "Purple"`，
`color = "#A855F7"`）。详见 [Yellow.md](./Yellow.md) 同系列说明。
