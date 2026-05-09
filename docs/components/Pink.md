# `<Pink />`

粉色高亮文字（Mac Preview 标注色 `#EC4899`）。

## 用法

```mdx
<Pink text="这段是粉色高亮" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要高亮的文字内容 |

## 输出

```html
<span style="color: #EC4899; font-weight: 600">这段是粉色高亮</span>
```

## 代码

`crates/widgets/src/components.rs::ColorComponent`（`name = "Pink"`，
`color = "#EC4899"`）。详见 [Yellow.md](./Yellow.md) 同系列说明。
