# R1 项目管理能力 —— 设计规格

> Status: approved design, not yet implemented
>
> Date: 2026-09-02
>
> 依据:`docs/omniproj-charter.md`(宪法)、`docs/requirements.md`(PRD)、
> `docs/product-reset-r0.md`(R0 shipped contract)。
> 本文修订 product-reset 契约的验收顺序(见 §1),新增 R1 功能需求;
> charter 原则(人主导、非侵入、事实派生、禁健康分)不变。

## 0. 问题

用户(产品所有者)的真实负载是**多项目并行 + 持续产生新方向**。R0 解决了「断档后如何
再入一个项目」;但日常**推进与跟踪**所需的管理能力有四个缺口(2026-09-02 对照代码核实):

| 缺口 | 现状(实测) |
|---|---|
| 预期日期不驱动关注 | `WorkItem.due` 存在但不参与任何派生:`ReviewReasonCode`(`review.rs:17`)只有 5 种,无逾期信号。任务到期,关注队列无反应 |
| 无看板视图 | `TaskBoard.tsx` 是平铺列表;数据模型已有 open/doing/done 三态,缺按列视图 |
| 无分类维度 | `WorkItem` 无 tags 字段(`project_state.rs:156` 逐字段核实) |
| 无跨项目任务视图 | 唯一跨项目 surface 是按 review reason 的项目队列 |

## 1. 契约修订(裁决)

`docs/product-reset-r0.md` 的 Acceptance 段要求「dogfood ≥5 项目、≥20 次 re-entry 之后
才扩 scope」,并警告不要用更多 planning surface 补偿。本文对该门槛做一次**有依据的修订**:

- **修订**:R1 的四项能力在 dogfood gate **之前**交付。理由:gate 要求的是「真实日用」,
  而产品所有者的日用形态就是多项目规划/排期/跟踪——缺这四项,dogfood 无法以真实负载开始。
  这是 gate 的前置条件,不是对 gate 失败的补偿。
- **不变**:R1 交付后立即进入 dogfood;R2 及之后(FR-V2/V3、跨项目看板编辑、更多可视化)
  仍被原 gate 锁定。
- **不变**:product-reset 的交互原则——项目页一个视觉终点、渐进披露、Index 行只答四问、
  Agent 提案零默选。R1 全部新界面都放在**已有披露层之内或其后**,不新增顶级导航
  (features must earn visible navigation through real dogfood)。

## 2. 目标 / 非目标

**目标**:
1. 预期日期(due)接入 Attend 闭环——逾期产生确定性 review reason,进队列、进菜单栏计数、进每日提醒。
2. 项目内任务支持看板(按状态三列)与时间分组两种替代视图。
3. 任务级 tags:录入、展示、过滤。
4. 跨项目「聚焦」聚合:逾期与今日到期任务一处可见。

**非目标**(R1 明确不做,理由附):
- **Gantt / 日历 / 工时估算 / start date**:单人科研场景估算维护成本高、易腐坏,违背
  low-maintenance state 原则;dogfood 出现真实痛感再议。
- **任务依赖关系、手动排序(rank)**:同上;列内排序用确定性规则(§4)。
- **项目级 tags**:跨项目聚合按任务 tags 已可覆盖大部分分类需求;项目分类推迟到 dogfood 后。
- **跨项目看板编辑**:编辑始终回到项目内,避免跨项目 revision 并发与心智复杂度(§7)。
- **key:value 结构化 tag**:纯字符串,不发明 DSL。

## 3. R1-1 逾期 → Attend(核心回路)

### 需求(FR-A4)

项目内存在 `status != done` 且 `due < today` 的 WorkItem 时,该项目获得 review reason
「有任务逾期」,随现有机制进入「需决策」组、菜单栏计数与每日提醒。

### 设计

- `ReviewReasonCode` 新增 `OverdueWork`。派生输入仍是纯人类状态(due 是用户手录),
  符合 `derive_review_reasons` 的既有原则(无 repository-observation 输入,活动/脏文件
  不影响结果)。
- **优先级序**(确定性,写入 evidence,不是健康分):
  `SourceUnavailable > CompleteSetup > NeedsCommitment > OverdueWork > ReviewAction > ScheduledReview`。
  逾期排在 commitment 处置请求之前:逾期是已违约的事实,review 请求是例行节奏。
- **evidence**:逾期条目列表(任务文本截断 60 字符 + due + 逾期天数),最多列 3 条,
  超出部分给计数(`…另有 N 条`)。
- **今天的定义**:due 是 naive date(`YYYY-MM-DD`)。判定用**用户本地日期**,由 desktop
  层计算后作为参数传入 core(core 保持确定性、可测试;不在 core 内取墙钟)。现有签名
  `derive_review_reasons(state, source, now: DateTime<Utc>, days)` 增加 `local_today: NaiveDate`。
- `REVIEW_RULE_VERSION` 从 `r0-v1` 升为 `r1-v1`(evidence 携带,历史可解释)。
- **due-soon(未来 N 天)不产生 reason**——避免提醒噪声;临期只在视图层可视化(§4/§5)。
- Current Commitment 绑定的 WorkItem 逾期同样计入(它也是任务);不额外加权。

### 验收

- 造 due=昨天 的 open 任务 → 项目进入「需决策」组,reason 为逾期且 evidence 含该任务;
  菜单栏计数 +1;将任务改 done 或清除 due → reason 消失。
- Waiting/Parked 项目的逾期任务**不**产生 reason(生命周期挂起即用户明示暂缓;
  Waiting 的 `review_at` 到期已有 ScheduledReview 兜底)。
- core 单测覆盖:边界日(due=today 不算逾期)、多条逾期 evidence 截断、优先级序。

## 4. R1-2 看板视图(项目内)

### 需求(FR-R1 扩展)

Planning 披露层内,任务列表支持 `列表 / 看板` 视图切换(默认列表,选择持久化到
`localStorage`,与语言偏好同机制)。看板按状态三列:open / doing / done。

### 设计

- **位置**:现有 Planning disclosure 内部,不新增顶级 surface,不改变项目页
  「当前下一步」的唯一视觉终点。
- **卡片**:任务文本、`?`未成形标记、due 徽标(逾期=danger tone、7 天内=warning tone,
  复用现有语义 tone 系统)、blocker 标记、tags(§5)、commitment 标识。
- **移动**:R1 用**显式控件**(卡片上的移动菜单/按钮),键盘完全可达——这是硬要求
  (现有 a11y 门禁)。HTML5 拖拽作为后续增强,单独验证 webview 内可靠性
  (仓库有原生拖拽修复史),不进 R1 验收。
- **Commitment 约束**:被 commitment 历史引用的 WorkItem 状态不可在看板直接改
  (既有契约:lifecycle status cannot be rewritten outside commitment transitions)。
  该卡片的移动控件禁用,并指向 commitment 处置(keep/revise/complete)。
- **done 列**:默认只显示最近 5 条 + 总数,展开查看全部(避免长期项目 done 列无限增长)。
- **列内排序**(确定性):逾期在前(按 due 升序),其后有 due 者按 due 升序,
  无 due 者按 `updated_at` 降序。
- 变更走现有 `update_task`/revision 乐观并发,冲突提示复用 `task.conflict`。

### 验收

- 三列计数与列表视图一致;键盘可将一张卡从 open 移到 doing;commitment 卡片移动控件
  禁用且给出指引;E2E 覆盖视图切换 + 移动 + 持久化。

## 5. R1-3 任务 tags

### 需求(FR-R5)

WorkItem 支持 0..8 个字符串 tag;录入、展示、按 tag 过滤(项目内)。

### 设计

- **模型**:`WorkItem.tags: Vec<String>`,`#[serde(default)]`。
- **迁移(关键)**:`ProjectStateDoc` 全链 `deny_unknown_fields`(`project_state.rs:155`
  实测),旧构建读到新字段会以 unknown-field 报错而非明确的版本拒绝。因此
  `DOCUMENT_SCHEMA_VERSION` **1 → 2**,走既有 stepwise 迁移骨架:v1→v2 迁移是纯
  no-op 重写(tags 缺省为空),非破坏;旧构建读 v2 文档得到明确的
  `UnsupportedSchema(2)`(既有检查,`project_state.rs:353`),符合「新版本文档被拒绝
  而非损坏」的既有原则。迁移必须在真实 store 副本上人工验证后才可合并(R0 验收同款流程)。
- **规范化**(写入时强制):trim、去首尾空白、NFC、空串拒绝、单 tag ≤24 字符、
  每任务 ≤8 个、去重(保序)。**不**强制小写(中文无大小写;英文保留用户原文,
  比较时 case-insensitive)。
- **UI**:任务行/卡片显示 tag chips(复用 `FilterChip` 语义组件族);编辑处输入 +
  项目内既有 tag 自动补全;Planning 披露层内按 tag 过滤(多选,AND 语义)。
- tags 不进入 review reason 派生,不影响排序——纯分类维度。

### 验收

- 加/删 tag 持久且重启可见;v1 旧文档打开即迁移到 v2 且任务数据无损(迁移测试);
- 过滤:选中 2 个 tag 只显示同时含两者的任务;
- 规范化:重复、空串、超长、超量在 core 层被拒绝并有单测。

## 6. R1-4 时间分组视图(排期的最小形态)

### 需求(FR-R6)

Planning 披露层第三种视图「按时间」:任务按 due 分组展示。

### 设计

- 分组(确定性,ISO 周、周一为界,本地日期):
  `逾期 / 今天 / 本周 / 下周 / 以后 / 未排期`;组内按 due 升序,未排期按 `updated_at` 降序。
- 纯派生视图,**零新数据、零新写路径**——只是 due 的另一种读法。
- done 任务不显示(时间视图回答「接下来什么到期」,不是回顾)。

### 验收

- 构造跨组用例断言分组与排序;周界(周日→周一)边界单测在 core 或纯函数层完成。

## 7. R1-5 跨项目聚焦(最小聚合)

### 需求(FR-A5)

一处可见所有 Active 项目的 `逾期 + 今天到期` 任务,按项目分组,点击进入该项目。

### 设计

- **位置**:Projects Index 顶部一条可折叠的「今日聚焦」区(默认折叠为一行摘要:
  「N 个项目共 M 条任务逾期或今日到期」;展开列明细)。不新增路由、不新增顶级导航
  ——它是 Index 回答「先碰哪个」的自然延伸,而非新 surface。
- **只读**:不提供跨项目编辑/状态变更;点击任务跳转到对应项目页(编辑回项目内,
  规避跨项目 revision 并发,也符合「队列 → 进入一个项目」的既有主循环)。
- 数据来自各项目已加载的 state 文档聚合,无新 IPC 概念(实现可加一个聚合查询命令,
  语义仍是读)。
- Waiting/Parked/Archived 项目不计入(与 §3 一致)。
- 空态:零逾期零今日到期时整条区域**不渲染**(product-reset 原则:零值状态省略)。

### 验收

- 两个 Active 项目各造一条逾期任务 → 摘要「2 个项目共 2 条」;展开可见明细并跳转;
  全部处理完后区域消失。E2E 覆盖。

## 8. 数据与一致性汇总

| 变更 | 影响 |
|---|---|
| `WorkItem.tags` | schema v1→v2,no-op 迁移;serde default;DTO/TS 类型同步 |
| `ReviewReasonCode::OverdueWork` | 纯派生,无存储变更;`REVIEW_RULE_VERSION` → `r1-v1` |
| 看板/时间视图/聚焦 | 纯视图,零存储变更 |
| 视图偏好(list/board/time) | `localStorage`,与语言偏好同级,不入 store |

并发:全部写路径复用既有 revision 乐观并发;无新锁。
非侵入硬约束不变:一切只写 `~/.omniproj`,源 repo 只读。

## 9. i18n / a11y

- 全部新 UI 中文优先、英文可切,新增 key 进 `I18nProvider` 双语表;
- 看板移动、tag 输入、聚焦区展开均键盘可达,进既有 axe/对比度/reduced-motion/
  200% 文本门禁;E2E 相应扩展;
- 逾期/临期颜色语义必须有非颜色冗余(文字「逾期 N 天」),过 grayscale 门禁。

## 10. 交付切分(每个独立 PR 回 dev,均过完整 pre-pr gate)

| PR | 内容 | 依赖 |
|---|---|---|
| R1a | `OverdueWork` reason + 优先级序 + Index/菜单栏/提醒接入 | 无 |
| R1b | schema v2 + tags 模型/迁移/DTO + 录入/展示/过滤 UI | 无 |
| R1c | 看板视图(含 commitment 锁定) | R1b(卡片显示 tags) |
| R1d | 时间分组视图 | 无(可与 R1c 并行) |
| R1e | 跨项目聚焦区 | R1a(复用逾期派生) |

实现顺序建议 R1a → R1b → (R1c ∥ R1d) → R1e;R1a 价值密度最高且零迁移风险,先行。
R1b 的迁移在真实 store 副本上人工验证后才可合并。

R1 全部合并后:注册 ≥5 个真实项目,进入 product-reset 契约的 dogfood 验收期,
R2 范围由 dogfood 数据决定。

## 11. 决策记录

1. **逾期进 Attend、临期不进**——提醒必须是已发生的事实,预测性提醒是噪声源。
2. **本地日期判逾期**——due 是用户口语义的「哪天前」,不是 UTC 时间戳。
3. **schema bump 而非兼容字段**——`deny_unknown_fields` 下旧构建的报错必须是明确的
   版本拒绝,不是解析错误;沿用既有迁移骨架。
4. **看板先按钮后拖拽**——键盘可达是门禁;webview 拖拽可靠性单独验证,不阻塞交付。
5. **commitment 卡片状态锁定**——不为看板便利打破 commitment 状态机的 append-only 契约。
6. **跨项目只读聚合**——编辑回项目内;跨项目编辑的复杂度(并发、上下文丢失)
   没有被任何真实痛感证明过。
7. **不做估算/依赖/Gantt**——维护成本会杀死 dogfood;先证明轻量形态不够,再加重。
