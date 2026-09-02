# OmniProj 产品需求(PRD)

> **状态说明（2026-08-31）：本文同时维护需求与当前实现边界。** `dev` 已交付 R0
> Projects Index / Project Overview / Current Commitment / 只读 Git observation；PR #19 的
> 开发版本已实现下文 MVP 纵向闭环，包括 16 周活动事实/静默排序、菜单栏待关注计数，
> 以及带系统钥匙串凭据、远程发送知情确认和结果数量校验的 Advance。仍未完成的是 macOS 签名公证、自动更新、Homebrew
> 分发，以及必须通过真实使用获得的 2–4 周 dogfood 证据。

> 依据 `docs/omniproj-charter.md`(宪法)。
> 形态/架构见 `docs/desktop-design.md`。本文只管**要什么、给谁、做到什么算数**,不管怎么实现。
> 原则:**简单、好用、明确、可产品化;自用 + 开源。** 反对复杂、反对 scope creep、反对建了没人用。

## 1. 目标与非目标

**目标**:让同时推进多个 git 项目的研究者,一屏看清「哪个在腐烂、下一步是什么、卡在哪」,被主动提醒,并能把「想不清、推不动」的条目一键让 agent 推到可执行——**agent 出建议,人拍板与执行**。

**非目标(明确不做)**:自主执行(不替你跑命令/改码/提交);通用 PKM/笔记库;通用 productivity/日历;git 历史通用可视化(交给 git/GitHub);多用户/云同步;可视化镀金。

## 2. 用户与场景

**主用户**:研究者,同时推进 3+ 个 git 项目,重度用 LLM(Claude/Codex),自己有判断力。**次用户(开源)**:同类多线程开发者/独立研究者。

**核心场景**(需求即从这些真实时刻推导):
- **S0 建项目与规划(人主导)**:注册一个 repo → 做初始规划 → 增加若干 task(设**预期完成日期**、记**可能的问题备注**)→ 维护 task list(open/doing/done)→ 显式选择一个 Current Commitment → task 与 commit 对账。这是核心活动,要好用。
- **S1 晨间重入**:打开 OmniProj → 一屏看到所有项目按腐坏度排,每个有 next-action / 进行中 task → **<1 分钟**决定今天先碰哪个、下一步是什么。
- **S2 主动提醒**:某项目静默越阈值 / 某卡点搁太久 → **原生系统通知**,不用我主动查。
- **S3 推进卡点**:一条 task「想不清」→ 点 Advance → agent 拆成候选子任务(或调研完善/对抗提问)→ 我挑选、采纳为正式 task。
- **S4 记录与对账**:进某项目详情 → git 提交时间线与我的 task 并排 → 我把「这 3 个 commit 完成了 task X」手动归属 → 记一条决策(含「决定不做 Y,因为…」)。

## 3. 核心概念与数据模型(需求层)

追踪单位 = **一个已注册的 git repo = 一个项目**。项目下有四类对象,全部落 markdown、随 `~/.omniproj` git 版本化:

| 对象 | 作者 | 内容 | 落地 |
|---|---|---|---|
| **Project** | 用户注册 | 一个 git repo 路径 | 现有 `meta.toml` |
| **Task**(planning item) | **用户 ground truth** | 文本、状态(open/**doing**/done)、`?`未成形、**预期完成日期**、**问题备注**、可选 proposal provenance、**关联 commit 列表** | `notes/next.md`(扩展) |
| **Commit 归属** | 用户手动 | task ↔ commit **多对一**(多个 commit 完成一个 task);未归属的 commit 仍在时间线上 | 记在 task 条目内 |
| **计划/决策日志** | 用户 | append-only 条目,状态 {planned/doing/done/**abandoned**},可选关联 commit,理由 | 新 `plan.md`(轻 ADR,不删只标) |
| **Advance 产物** | **AI derivative** | 拆解/调研/提问结果 | `auto/`;用户采纳才升为 Task |

**铁律**(charter 原则3):AI 产物一律先落 `auto/` derivative;**只有用户显式采纳才写入 `notes/` ground truth**;AI 永不覆盖用户内容。

## 4. 功能需求(按三层 + 验收标准)

### 4.1 关注 Attend(#4)—— MVP 核心
- **FR-A1 项目总览**:一屏列出所有项目,每个显示:腐坏度(距上次提交/活动,**阈值旁注**)、一行 next-action、16 周 commit sparkline、卡点标记。**按腐坏度排序**(中性事实,禁优先级排名/健康分)。
- **FR-A2 受控 push 提醒**:**默认每天(daily)提醒一次**——每日汇总当前该关注的项目(静默/卡点)→ 原生系统通知。**节奏与阈值用户可见、可调、可关**;不轰炸、不视觉打断。
- **FR-A3 菜单栏常驻**:图标旁以 macOS 原生标题显示「N 个待关注」(零时隐藏数字),点开即主屏；菜单项与 tooltip 使用同一计数。
- **FR-A4 逾期驱动关注(R1)**:Active 项目内存在 `due < 本地今日` 且未完成的 task 时,产生确定性 review reason「有任务逾期」,进入需决策组、菜单栏计数与每日提醒;Waiting/Parked 不计入。临期(未来 N 天)不产生 reason,只在视图层可视化。
- **FR-A5 跨项目聚焦(R1)**:Projects Index 顶部可折叠「今日聚焦」区,聚合所有 Active 项目的逾期 + 今日到期任务,按项目分组、只读、点击跳转对应项目;零条目时整区不渲染。
- **验收**:冷启动到「知道先碰哪个 + 下一步」**< 60 秒**;主指标 re-entry 时间 15–25 分 → **< 5 分**。

### 4.2 记录 Record(#1 #2)—— 人主导规划与 Git 对账
- **FR-R1 人主导 task 管理(核心活动)**:建项目 → 做规划 → 增删 task,维护:状态(open/doing/done)、`?`未成形、**预期完成日期**、**问题备注**、卡点。Task 是规划清单；用户可把其中一条显式提升为唯一 Current Commitment，之后其有效状态由 commitment 生命周期派生。长对话式 discussion 不进入当前 MVP，避免重造聊天/PKM。
- **FR-R2 Git 对账**:项目详情展示提交时间线及轻量 commit topology summary；用户可把**一个或多个 commit 归属到一个 task**(多对一)，并可解除/改绑。当前不宣称提供 gitk 式完整 branch-lane flow graph。
- **FR-R3 计划/决策日志**:`plan.md`(独立文件)append-only 记规划与决策,含**「决定不做」**(标 abandoned 不删),可选关联 commit。
- **FR-R4 非侵入(硬约束)**:只**读** repo 的 git 数据,**绝不写/改 repo**(§5)。
- **FR-R5 任务 tags(R1)**:task 支持 0..8 个字符串 tag(单个 ≤24 字符,写入时 trim/NFC/去重,比较大小写不敏感);录入带项目内自动补全,展示为 chips,项目内可按 tag 多选过滤(AND)。tag 不参与 review reason 派生与排序,纯分类维度;不做 key:value 结构。
- **FR-R6 任务视图(R1)**:Planning 披露层内 task 支持三种视图,默认列表、选择本地持久化:①**看板**——按状态三列(open/doing/done),移动用键盘可达的显式控件(拖拽为后续增强),commitment 绑定的 task 状态锁定、指向 commitment 处置,done 列默认收纳(最近 5 条 + 总数);②**按时间**——按 due 分组(逾期/今天/本周/下周/以后/未排期,ISO 周、本地日期),done 不显示;③现有列表。三种视图零新数据、共用同一写路径。
- **验收**:用户能建项目并维护带状态/日期/备注的 task list；能把 task 提升为 Current Commitment；能在提交时间线上把 ≥1 commit 归属、解除或改绑到 task；能记「决定不做 X」永久可查。R1:能给 task 加 tag 并过滤;看板三列计数与列表一致且键盘可移动;时间视图分组正确。

### 4.3 推进 Advance(#3)
- **FR-V1 拆解(MVP)**:对一条 task/想法,agent 产出 3–6 条**具体可执行**候选子任务;用户可一键采纳为正式 task。provider/model 在 app 内配置；远程调用先取得发送 task 文本与问题备注的知情确认；API key 只存系统钥匙串。格式不合格只做一次有界重试，仍不合格则报错且不写 proposal。
- **FR-V2 调研+完善**(后续):对一个想法读 repo + **联网**做背景调研,完善成清晰需求/spec,落 `auto/`。
- **FR-V3 对抗提问**(后续):只把想不清处问清楚(现 clarify),不直接产任务。
- **共同护栏**:三种都是**人在环**——agent 出建议,用户逐条选择后采纳;agent **绝不**自主执行或直接写 ground truth。采纳后的 Task 保留 proposal provenance。
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
- **人主导 task 管理(FR-R1)—— 核心**:增删 task、状态 open/doing/done、预期完成日期、问题备注，并显式提升一条 task 为 Current Commitment。
- **git 对账基础(FR-R2)**:提交时间线 + 把 commit 归属、解除、改绑到 task(多对一)；轻量 topology summary 只表达 parent/ref/merge，不冒充完整 branch-lane graph。
- **Attend(FR-A1/A2/A3)**:总览 + 每日提醒 + 菜单栏。
- **Advance 拆解(FR-V1)**:单一模式。

**R1(项目管理能力,dogfood 的前置;设计见 `docs/superpowers/specs/2026-09-02-r1-project-management.md`)**:
- **逾期驱动关注(FR-A4)** + **跨项目聚焦(FR-A5)**:让「预期完成日期」真正驱动 Attend 闭环。
- **任务 tags(FR-R5)** + **看板/时间视图(FR-R6)**:项目内规划、排期、分类、跟踪的日用形态。
- 交付切分:R1a 逾期 reason → R1b tags(含 schema v2 迁移)→ R1c 看板 ∥ R1d 时间视图 → R1e 跨项目聚焦;每项独立 PR。
- R1 明确**不做**:Gantt/日历/工时估算/start date、任务依赖、手动排序、项目级 tag、跨项目编辑。

**门槛**:MVP+R1 真被日用 2–4 周、覆盖 ≥5 个真实项目并记录 ≥20 次 re-entry event 才继续扩展(R2:FR-V2/V3、跨项目看板编辑等)。R1 是该门槛要求的「真实日用」的前置条件——用户日用形态就是多项目规划/排期/跟踪——而非对门槛失败的补偿。当前 UI 的 re-entry timer 将事件写入本地 `dogfood/reentry-events.jsonl`；统计解释见 `docs/dogfood.md`。

## 7. 细节决策
1. **Task 状态**:open / **doing** / done(+ `?`未成形)。要 doing 中间态。
2. **提醒默认**:**每天(daily)** 一次日汇总;节奏/阈值可调可关。
3. **FR-V2 调研**:**允许联网**(读 repo + 上网)。
4. **`plan.md`**:**独立文件**,与 task 分开。
5. **非侵入(硬约束)**:对原始 repo 零侵入零修改,只读其 git;一切落 `~/.omniproj`(见 §5)。
6. **逾期判定(R1)**:due 按**用户本地日期**比较(due < today 为逾期,due = today 不算);临期不产生提醒。Waiting/Parked 项目的逾期不进关注队列(生命周期挂起即用户明示暂缓)。
7. **tags 存储(R1)**:`WorkItem.tags` 落项目 state 文档,schema v1→v2(既有迁移骨架,no-op 迁移;旧构建对 v2 文档得到明确版本拒绝而非解析错误)。
