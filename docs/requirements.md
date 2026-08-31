# OmniProj 产品需求(PRD)

> **状态说明（2026-08）：本文是后续 MVP 产品愿景，不是当前已交付功能清单。** 当前桌面版实现的是 README 所述 R0：Projects Index、Project Overview、commitment 与只读 Git observation。本文中的 task 管理、通知、Advance、Git flow graph 等仍属规划，不能据此判断当前版本已经提供。

> 依据 `docs/omniproj-charter.md`(宪法)。
> 形态/架构见 `docs/desktop-design.md`。本文只管**要什么、给谁、做到什么算数**,不管怎么实现。
> 原则:**简单、好用、明确、可产品化;自用 + 开源。** 反对复杂、反对 scope creep、反对建了没人用。

## 1. 目标与非目标

**目标**:让同时推进多个 git 项目的研究者,一屏看清「哪个在腐烂、下一步是什么、卡在哪」,被主动提醒,并能把「想不清、推不动」的条目一键让 agent 推到可执行——**agent 出建议,人拍板与执行**。

**非目标(明确不做)**:自主执行(不替你跑命令/改码/提交);通用 PKM/笔记库;通用 productivity/日历;git 历史通用可视化(交给 git/GitHub);多用户/云同步;可视化镀金。

## 2. 用户与场景

**主用户**:研究者,同时推进 3+ 个 git 项目,重度用 LLM(Claude/Codex),自己有判断力。**次用户(开源)**:同类多线程开发者/独立研究者。

**核心场景**(需求即从这些真实时刻推导):
- **S0 建项目与规划(人主导)**:注册一个 repo → 做初始规划 → 增加若干 task(设**预期完成日期**、记**可能的问题备注**、展开**讨论**)→ 维护 task list(状态 open/doing/done)→ task 对应到 **git flow graph**。这是核心活动,要好用。
- **S1 晨间重入**:打开 OmniProj → 一屏看到所有项目按腐坏度排,每个有 next-action / 进行中 task → **<1 分钟**决定今天先碰哪个、下一步是什么。
- **S2 主动提醒**:某项目静默越阈值 / 某卡点搁太久 → **原生系统通知**,不用我主动查。
- **S3 推进卡点**:一条 task「想不清」→ 点 Advance → agent 拆成候选子任务(或调研完善/对抗提问)→ 我挑选、采纳为正式 task。
- **S4 记录与对账**:进某项目详情 → git 提交时间线与我的 task 并排 → 我把「这 3 个 commit 完成了 task X」手动归属 → 记一条决策(含「决定不做 Y,因为…」)。

## 3. 核心概念与数据模型(需求层)

追踪单位 = **一个已注册的 git repo = 一个项目**。项目下有四类对象,全部落 markdown、随 `~/.omniproj` git 版本化:

| 对象 | 作者 | 内容 | 落地 |
|---|---|---|---|
| **Project** | 用户注册 | 一个 git repo 路径 | 现有 `meta.toml` |
| **Task**(next-action) | **用户 ground truth** | 文本、状态(open/**doing**/done)、`?`未成形、**预期完成日期**、**问题备注**、**讨论**、可选卡点、**关联 commit 列表** | `notes/next.md`(扩展) |
| **Commit 归属** | 用户手动 | task ↔ commit **多对一**(多个 commit 完成一个 task);未归属的 commit 仍在时间线上 | 记在 task 条目内 |
| **计划/决策日志** | 用户 | append-only 条目,状态 {planned/doing/done/**abandoned**},可选关联 commit,理由 | 新 `plan.md`(轻 ADR,不删只标) |
| **Advance 产物** | **AI derivative** | 拆解/调研/提问结果 | `auto/`;用户采纳才升为 Task |

**铁律**(charter 原则3):AI 产物一律先落 `auto/` derivative;**只有用户显式采纳才写入 `notes/` ground truth**;AI 永不覆盖用户内容。

## 4. 功能需求(按三层 + 验收标准)

### 4.1 关注 Attend(#4)—— MVP 核心
- **FR-A1 项目总览**:一屏列出所有项目,每个显示:腐坏度(距上次提交/活动,**阈值旁注**)、一行 next-action、16 周 commit sparkline、卡点标记。**按腐坏度排序**(中性事实,禁优先级排名/健康分)。
- **FR-A2 受控 push 提醒**:**默认每天(daily)提醒一次**——每日汇总当前该关注的项目(静默/卡点)→ 原生系统通知。**节奏与阈值用户可见、可调、可关**;不轰炸、不视觉打断。
- **FR-A3 菜单栏常驻**:图标显示「N 个待关注」,点开即主屏。
- **验收**:冷启动到「知道先碰哪个 + 下一步」**< 60 秒**;主指标 re-entry 时间 15–25 分 → **< 5 分**。

### 4.2 记录 Record(#1 #2)—— 人主导规划,git flow graph 对账
- **FR-R1 人主导 task 管理(核心活动)**:建项目 → 做规划 → 增删 task,维护:状态(open/doing/done)、`?`未成形、**预期完成日期**、**问题备注**、**讨论**、卡点。UI 要**好用**(first-class,不是极简一行)。这是产品核心价值,不是负担——工具支持人把它做好。
- **FR-R2 git flow graph 对账**:项目详情展示 git flow graph(分支 + 提交图)作为「实际」线;task **叠加/对应**其上——用户可把**一个或多个 commit 归属到一个 task**(多对一),一眼看「计划 vs 实际」。并排为默认,归属为可选增量。
- **FR-R3 计划/决策日志**:`plan.md`(独立文件)append-only 记规划与决策,含**「决定不做」**(标 abandoned 不删),可选关联 commit。
- **FR-R4 非侵入(硬约束)**:只**读** repo 的 git 数据,**绝不写/改 repo**(§5)。
- **验收**:用户能建项目并维护带状态/日期/备注/讨论的 task list;能在 git flow graph 上把 ≥1 commit 归属到一个 task;能记「决定不做 X」永久可查。

### 4.3 推进 Advance(#3)
- **FR-V1 拆解(MVP)**:对一条 task/想法,agent 产出 ≥3 条**具体可执行**候选子任务;用户可一键采纳为正式 task。
- **FR-V2 调研+完善**(后续):对一个想法读 repo + **联网**做背景调研,完善成清晰需求/spec,落 `auto/`。
- **FR-V3 对抗提问**(后续):只把想不清处问清楚(现 clarify),不直接产任务。
- **共同护栏**:三种都是**人在环**——agent 出建议,用户决策与采纳;agent **绝不**自主执行或直接写 ground truth。可见的调用计数做自我监控(charter §8 反指标「推荐诱发依赖」)。
- **验收**:一条标「?未成形」的 task,经 FR-V1 一次得到 ≥3 条可采纳子任务。

## 5. 非功能需求
- **简单**:功能通不过「明早我会为它打开吗」就该是个 markdown,不是功能。命令/界面元素宁少勿多。
- **本地优先**:状态全在本地 `~/.omniproj`,markdown+git,可读可移植;LLM API 可远端,输出落地本地。
- **桌面**:Tauri,原生通知/菜单栏;先 macOS(签名公证),Linux/Windows 后补。
- **可产品化/开源**:点图标即用,注册项目走 UI(CLI 最小保留 `add`);GitHub Release + Homebrew cask。
- **非侵入式 repo(硬约束)**:对原始 repo **零侵入、零修改**——不写入、不加文件(连 `.gitignore`/hook 都不加)、不改配置。所有记录/分析/关联只发生在 `~/.omniproj` 内(只**读** repo 的 git 数据)。删 `~/.omniproj` 即完全卸载,repo 无任何痕迹。
- **数据安全**:不上传状态;LLM 调用可远端但输出落地本地。

## 6. 优先级 / MVP 切分
**MVP(可 dogfood 的最小核)**:
- **人主导 task 管理(FR-R1 全)—— 核心,必须做好**:增删 task、状态 open/doing/done、预期完成日期、问题备注、讨论。
- **git 对账基础(FR-R2 基础版)**:提交时间线 + 把 commit 归属到 task(多对一)。**完整 git flow graph(分支图叠加)作深化**。
- **Attend(FR-A1/A2/A3)**:总览 + 每日提醒 + 菜单栏。
- **Advance 拆解(FR-V1)**:单一模式。

**门槛**:MVP 真被日用(真实项目 3–4 周)才继续——完整 git flow graph、FR-R3 决策日志、Advance 扩展(FR-V2 联网调研 / FR-V3 对抗提问)。对应 `desktop-design.md` 里程碑。

## 7. 细节决策
1. **Task 状态**:open / **doing** / done(+ `?`未成形)。要 doing 中间态。
2. **提醒默认**:**每天(daily)** 一次日汇总;节奏/阈值可调可关。
3. **FR-V2 调研**:**允许联网**(读 repo + 上网)。
4. **`plan.md`**:**独立文件**,与 task 分开。
5. **非侵入(硬约束)**:对原始 repo 零侵入零修改,只读其 git;一切落 `~/.omniproj`(见 §5)。
