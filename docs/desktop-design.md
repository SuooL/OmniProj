# OmniProj 桌面版 —— 产品设计 + 规划

> 依据:`docs/omniproj-charter.md`(宪法)+ `docs/requirements.md`(PRD)。
> 目标(用户 2026-08-10):**简单、好用、明确、可产品化;自用 + 开源**。

## 0. 形态裁决(已定)

**只做桌面端(Tauri),不做 web。** 理由:①#4「及时提醒」/§4d 受控 push 需要**原生系统通知 + 菜单栏常驻**——浏览器送不出;②点图标即开 >> 敲命令+开浏览器+管 server,更可产品化;③后端已是 Rust,Tauri 阻抗最小、复用现有 React 前端、体积小;④本地数据原生读,绕开 localhost web 的安全面(可退役 DNS-rebinding 护栏)。

**架构简化**:Tauri 原生 **IPC**(前端 `invoke()` 直调 Rust 命令)→ **退役 axum HTTP server 层**;保留底层数据核(omniproj-core / capture / distill provider)+ React 前端。

## 1. 一句话产品

**OmniProj 是一个 git 锚定的科研推进器桌面 app:一屏看清你所有在研课题「哪个在腐烂、下一步是什么、卡在哪」,课题静默会主动提醒你,想不清的条目一键让 agent 拆到可执行——agent 出建议,你拍板。**

## 2. MVP 形态(最小可 dogfood 的核)

**主屏 = 晨间屏(关注 Attend)**:一个窗口列出所有在研课题,每张卡:
- 腐坏度(距上次提交/活动多久)—— 阈值旁注(§8 护栏 i,禁健康分)
- 一行 **next-action** + 卡点(用户手写,ground truth)
- 16 周 commit sparkline
- **按腐坏度排序**(中性事实,非优先级排名)

**菜单栏常驻**:图标显示「N 个课题待关注」;某课题静默越阈值 → **原生系统通知**(受控 push,阈值可调可关)。

**人主导 task 管理(核心活动)**:点进项目 → 富 task 列表:增删 task、状态 open/doing/done、预期完成日期、问题备注、讨论。这是核心价值,**不是"极简一行"**——工具支持人把规划做好(charter §4c 纠正版)。

**git 对账基础**:项目详情里 git 提交时间线 + 把 commit 归属到 task(多对一),一眼看计划 vs 实际。**完整 git flow graph(分支图叠加)作深化**。

**自动派生**:腐坏度 / 活动 / 提交这些 git 已知的自动算,不用你手录。

> MVP = Attend 总览 + **人主导 task 管理 + git 对账基础** + Advance 拆解 → 即可每天用。完整 flow graph / 决策日志 / Advance 扩展 gated 在 MVP 真被日用之后。

## 3. 三层 → 界面

| 层 | 界面 | 里程碑 |
|---|---|---|
| **关注 Attend** | 主屏晨间屏 + 菜单栏 + 原生通知 | M0–M1 |
| **记录 Record** | 项目详情:**人主导 task 管理**(状态/日期/问题备注/讨论)+ git flow graph 对账(task↔commit 多对一,*实际*⇔*意图*)+ 计划/决策日志(`plan.md`,含「决定不做」abandoned) | M2 基础 → M3 flow graph |
| **推进 Advance** | 卡住/未成形条目上的「Advance」按钮 → agent 拆成候选下一步(落 `auto/` derivative,用户挑选→提升为 ground truth)。**agent 推荐,人决策** | M4 |
| *形态 Form* | Tauri 窗口 + 菜单栏,React/Tailwind 可视化 | 贯穿 |

## 4. 数据模型(最小,markdown + git,复用)

- `notes/next.md`(**复用+扩展**)—— 用户 ground truth:task(文本 / 状态 open·doing·done / `?`未成形 / 预期完成日期 / 问题备注 / 讨论 / 卡点 / 关联 commit 列表)。
- `auto/`(**复用**)—— agent derivative:拆解、澄清、调研产物。用户采纳才写回 `notes/`。
- `plan.md`(**独立新文件**)—— 项目规划 + 决策日志,append-only,可标「abandoned」(不删只标,ADR superseded),可锚 git commit。
- `~/.omniproj` git store(**复用**)—— planned-vs-actual 的历史基底;每次写入 = 独立可回退 commit。

## 5. 架构(Tauri + 复用)

```
Tauri 桌面 app
├─ 前端(webview):React 19 + Vite + Tailwind + TanStack Query（复用现有前端，fetch→invoke）
├─ Tauri Rust 后端:IPC 命令 handler（薄层，取代 axum handlers）
│    └─ 调用 ↓
├─ omniproj-core   路径/notes/store 自版本化（复用）
├─ omniproj-capture git 解析 + session（复用；需补结构化逐提交历史）
└─ omniproj-distill provider 管线（复用；Advance 用；砍蒸馏/opinion/deep）
```

原生能力:系统通知(受控 push)、菜单栏 tray、文件系统直读(`~/.omniproj`、用户 repo、`~/.claude`/`~/.codex`)。

## 6. 代码留 / 砍(✅ 已执行 2026-08-10)

> 已在 `feature/desktop-pivot` 执行本节砍除清单。CLI 侧采用**渐进收敛**:先砍掉 api/daemon/ipc
> 三个 crate + 后台蒸馏/opinion/eval/doctor 等命令,**暂留** capture/notes 侧工具命令
> (`list`/`remove`/`digest`/`search`/`recall`/`note`/`next`/`clarify`/`stats`/`providers`/`init`),
> 待桌面 M2/M3 接管后再收敛到「只剩 `add`」——避免在桌面重实现前凭空 strand 能力。详见 CHANGELOG。

**留(核心,复用 ~80%)**:`omniproj-core`(paths/notes/store)、`omniproj-capture`(git/session)、`omniproj-distill` 的 `provider.rs` + verify + clarify、`omniproj-index`、React 前端(portfolio/sparkline)。

**砍(✅ 已删)**:`omniproj-api` axum server 层(→ Tauri IPC)、`omniproj-daemon`(→ Tauri 后台)、`omniproj-ipc`、opinion / user-model / second-opinion / deep-pipeline / curate / eval / doctor / install-service / reconcile / mcp。CLI(`omniproj-cli`)**最终**只保留 `add`(注册项目),其余走桌面 UI(当前渐进保留见上)。

**新建(窄)**:Tauri 外壳 + IPC 命令、菜单栏/每日通知、**task 模型扩展**(状态 doing / 预期完成日期 / 问题备注 / 讨论)、结构化逐提交历史、**git flow graph 视图 + task↔commit 对应**、`plan.md` 决策日志(abandoned 标记)、Advance 拆解写路径(现 clarify 刻意不收敛,需新 prompt)。

## 7. 里程碑(到开源可发布)

| M | 内容 | 产出 | dogfood |
|---|---|---|---|
| **M0** | Tauri 外壳 + 复用 React 主屏,IPC 读真实数据(fetch→invoke) | 能打开的桌面窗口,总览跑真实项目 | — |
| **M1** | 菜单栏常驻 + 每日提醒(受控 push,节奏/阈值可调) | Attend 层完整 | — |
| **M2** | 项目详情:**人主导 task 管理**(增删/状态 open·doing·done/预期完成日期/问题备注/讨论)+ git 提交时间线 + commit→task 归属(多对一) | Record 基础(规划 + 对账),**核心** | — |
| **M3** | Advance 拆解(agent 拆成候选子任务,人在环采纳) | Advance MVP | **MVP 完成 → 真实项目跑 3–4 周** |
| — | **门槛**:真被日用才继续;否则回头改"记/推"的形态 | | |
| **M4** | 完整 git flow graph(分支图叠加,task 对应)+ `plan.md` 决策日志(含 abandoned) | Record 深化 | |
| **M5** | Advance 扩展:FR-V2 联网调研 / FR-V3 对抗提问 | Advance 扩展 | |
| **M6** | macOS 签名/公证 + 自动更新(后续 Linux/Windows),开源发布 | 可分发产品 | |

## 8. 决策

1. **代码砍除照准**(§6):按清单砍,简单优先;存量全在 git 历史,可回退。
2. **新建 `plan.md` 走轻 ADR**:计划/决策日志与 `decisions.md`(AI 蒸馏)职责分开;条目 append-only、可锚 git commit、`abandoned` 只标不删(superseded 风格)。
3. **CLI 最小保留 `add`**(注册项目):其余以桌面 UI 为主入口;不保留 22 命令那套。
4. **发布先只 macOS**:先把签名/公证/自动更新跑通,Linux/Windows 后补。

> 发布渠道:GitHub Release + Homebrew cask。
