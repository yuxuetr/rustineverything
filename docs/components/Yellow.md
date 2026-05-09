# `<Yellow />`

黄色高亮文字（Mac Preview 标注色 `#EAB308`）。

## 用法

```mdx
<Yellow text="这段是黄色高亮" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要高亮的文字内容 |

## 输出

```html
<span style="color: #EAB308; font-weight: 600">这段是黄色高亮</span>
```

## 代码

`crates/widgets/src/components.rs::ColorComponent`（`name = "Yellow"`，
`color = "#EAB308"`）。

## 同系列

`<Green text="…" />` / `<Blue text="…" />` / `<Pink text="…" />` /
`<Purple text="…" />` 共享同一 `ColorComponent` 实现，只是颜色 hex
不同。颜色取自 Tailwind 500 级别的 `yellow / green / blue / pink / purple`。
