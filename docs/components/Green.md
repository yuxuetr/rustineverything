# `<Green />`

绿色高亮文字（Mac Preview 标注色 `#22C55E`）。

## 用法

```mdx
<Green text="这段是绿色高亮" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要高亮的文字内容 |

## 输出

```html
<span style="color: #22C55E; font-weight: 600">这段是绿色高亮</span>
```

## 代码

`crates/widgets/src/components.rs::ColorComponent`（`name = "Green"`，
`color = "#22C55E"`）。详见 [Yellow.md](./Yellow.md) 同系列说明。
