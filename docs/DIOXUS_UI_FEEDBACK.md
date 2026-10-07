# dioxus-ui 实践反馈记录

> 在 rustineverything.app 迁移到 [`dioxus-shadcn`](https://github.com/yuxuetr/dioxus-ui) 的过程中（计划见 [`DIOXUS_UI_MIGRATION.md`](DIOXUS_UI_MIGRATION.md)），
> 记录组件库本身的问题与改进点，之后整理成 dioxus-ui 的 issue / RFC。

## 记录规则

- **只记组件库的问题**：bug、API 难用、文档缺失、SSR/hydration 行为、样式可移植性。本项目自己的问题不记在这里。
- **先最小复现，再记录**：尽量给出能在 dioxus-ui 的 `examples/` 里复现的最小 RSX。
- **临时处理要可追溯**：本项目里为某个问题写的临时处理，代码注释标注编号（`// dioxus-ui FB-03`），上游修复后搜索编号删除。
- **状态**：`open`（已记录）→ `reported`（已提 issue，附链接）→ `fixed x.y`（上游已发版）→ `removed`（本项目临时处理已删）。也可能是 `wontfix`（评估后认为是本项目的用法问题）。

## 条目模板

```markdown
### FB-NN 一句话标题

- 组件 / 版本：
- 发现于：U? — 文件:行号
- 现象：
- 复现：
- 影响：
- 本项目的临时处理：
- 建议的上游修复：
- 状态：open
```

## 条目

### FB-01 `@source` 绝对路径在 Docker / CI 构建中不可移植

- 组件 / 版本：`dioxus-shadcn-cli`（`dxui init`）0.6.0
- 发现于：迁移规划阶段（U1 前）
- 现象：`dxui init` 写入 `@source "<本机 ~/.cargo/registry 绝对路径>/dioxus-shadcn-0.6.0/src"`。本项目在 Docker 构建阶段和 CI 中编译 Tailwind，那里的 registry 路径不同或不存在，Tailwind 不报错，只是组件用到的类名不生成，表现为组件**没有样式**。
- 复现：在一台机器上 `dxui init`，把生成的 css 提交，在另一台机器或 Docker 中 `npx @tailwindcss/cli -i … -o …`，产物里没有 `bg-primary` 等组件类名。
- 影响：所有依赖 crate 而非复制源码的用户，只要 CSS 不是在本机编译，都会踩到；错误是静默的。
- 本项目的临时处理：`scripts/tw_sources.sh` 在构建时用 `cargo metadata` 生成 `@source` 行（见迁移计划 D3）。
- 建议的上游修复：提供 `dxui css-sources`（只打印或写出当前解析到的 `@source` 行，供构建脚本调用），并在 README 的「Depend on the crate」一节说明 CI / Docker 场景；或者随 crate 附带一份预生成的类名清单（safelist）。
- 状态：open
