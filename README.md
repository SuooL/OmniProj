# OmniProj

**一本能自己对上 git 的笔记本。** 一个本地优先的桌面 app：每个项目一页带层级的步骤清单，旁边是这个仓库真实的 git 活动。

```
前列腺癌穿刺优化 MRI 预测 <FM>          dev · 12 commits · 3 天前
▁▃▅▂▁▁▇▅▃▁▂▆▅▃▁▁
────────────────────────────────────────
 1 ✓ 方案拟定
 2 ✓ Down Stream Task 定义
 3 ✓ 报告提取
     ✓ OCR
     ✓ 结构化批量
 4 ✓ CT 数据 DICOM 批处理转换管线
 5 □ 论文框架搭建                    09-20
 + 新增一步…
```

产品定义见 [`docs/omniproj-charter.md`](docs/omniproj-charter.md)，界面约定见 [`docs/design.md`](docs/design.md)。

---

## 能做什么

- **带层级的清单。** 顶层步骤自动编号，子步骤跟着父步骤走。回车新起一行，Tab 缩进，Shift+Tab 退回上层，⌥↑/⌥↓ 调序，点文字就地改。
- **git 活动自动派生。** 一年的每日提交热图、提交时间线、分支图，全部只读；步骤可以关联 commit（多对一）。
- **可选的 AI 拆解。** 一步想不清就让它给候选，你勾选采纳成子步骤。不勾就不写。
- **本地优先。** 状态全在 `~/.omniproj`，人类可读，本身是个 git 仓库，每次写入可回退。

## 不做什么

自主执行的 agent、通用笔记库、通用 git 客户端、优先级/健康分排序、多人协作、云同步。理由见 charter §6。

## 平台与隐私

- **平台：** Tauri 桌面 app，macOS 为验收平台（CI 跑 Linux）。Rust + React。
- **对你的仓库零写入。** 只跑只读 git 命令；不写文件、不加 hook、不改配置。移动或改名不会毁掉项目，重新关联即可。
- **数据在本地。** `~/.omniproj`（用 `OMNIPROJ_HOME` 可改）。AI 拆解只在你显式同意后发送该步骤的文字与备注；API key 存系统钥匙串，不写进数据目录。

## 键盘

清单里（点开某一行的文字后）：

| 键 | 作用 |
|---|---|
| `Enter` | 在下面新起一行 |
| `Tab` / `Shift+Tab` | 缩进成子步骤 / 退回上一层 |
| `⌥↑` / `⌥↓` | 与上/下一个同级步骤交换位置 |
| `↑` / `↓` | 移到上/下一行继续编辑 |
| `Backspace`（空行） | 删除这一行 |
| `Esc` | 放弃本次改写 |

全局：`Cmd/Ctrl+F` 聚焦搜索，`Cmd/Ctrl+N` 新建项目，`Cmd/Ctrl+R` 重新观测仓库。

## 本地文件

```
~/.omniproj/
  meta.toml                        # 项目登记表
  projects/<ProjectId>/
    meta.toml                      # 登记信息 + 仓库位置
    notes/project.md               # 步骤清单：TOML front matter + 你的正文
    auto/advance/                  # AI 候选（未采纳前不进清单）
    cache/r0-observation.json      # 最近一次仓库观测（派生，可重建）
```

`notes/project.md` 的正文是逐字节保留的：OmniProj 只改 front matter，从不重写你的文字。

## 写入的可靠性

每次改动都是一次显式的、带版本号校验的写入。如果保存时发现文件已被更新，OmniProj 会拉回最新状态并提示，不会盲目覆盖；如果状态已经落盘但审计提交失败，它会重新加载已保存的状态，绝不重发你的改动。

## 开发

```bash
# Rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked

# 前端（crates/omniproj-desktop/web）
npm ci
npm test               # Vitest
npm run build          # tsc + vite，重建内嵌的 dist/
npm run test:e2e       # Playwright（需先 npx playwright install chromium）

# 桌面 app（crates/omniproj-desktop）
cargo tauri dev        # 开发运行；直接 cargo run 会得到空白窗口
cargo tauri build      # 打包
```

`crates/omniproj-desktop/web/dist/` 是**提交进仓库的**（会嵌进二进制，`cargo install` 的用户没有 npm）。改完前端记得重新 build 再提交。

CI 在每个到 `dev`/`main` 的 PR 上跑前端单测/构建、Playwright e2e、以及 Rust workspace 三个 job。

### 仓库结构

```
crates/
  omniproj-core/       状态文档、步骤树、校验、审计写入
  omniproj-capture/    只读 git 观测（提交、分支、热图分桶）
  omniproj-distill/    AI 拆解的 provider 层
  omniproj-index/      跨项目索引
  omniproj-desktop/    Tauri 命令层 + React 前端（web/）
  omniproj-cli/        命令行入口
```

### 分支约定

`main` / `dev` 双干线，PR-only，CI 门禁。实际开发在 `feature/<topic>`（从最新 `dev` 切出），完成后 PR 回 `dev`。提交用 Conventional Commits。
