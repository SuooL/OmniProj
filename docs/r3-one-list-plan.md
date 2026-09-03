# R3 —— 一份清单：消除「承诺 / 任务」的概念分裂

> 依据：`docs/omniproj-charter.md` §3（三层）、§4c（人主导规划）、§8（反指标）。
> 触发：用户 2026-09-03 的五个问题——「更多操作那两个按钮有必要吗」「任务管理一塌糊涂」「人工决策是什么」「人工承诺是什么」「这工具到底在做什么」。
> 状态：**方案待审，未动代码。**

---

## 0. 诊断（一句话）

不是功能不够，是**内部数据模型被原样贴成了 UI 文案**。同一件事（"我接下来做什么"）在界面上有两个实体、两套录入、两个名字；另一件事（"我为什么这么定"）跟任务长得一模一样却是另一个列表。

已命中宪法 §8 反指标：**「明早不会为它开 app」**。

---

## 1. 关键发现：后端早就是一份清单

| 事实 | 证据 |
|---|---|
| 任务清单渲染的就是 `state.work_items` | `crates/omniproj-desktop/src/mvp.rs:632` |
| 「当前承诺」只是指针 `current_next_action_id` 指向其中一条 | `mvp.rs:654` |
| 在承诺表单手写文字会**往 `work_items` push 一条新的** | `crates/omniproj-core/src/project_state.rs:1124-1127` |
| 提升任务为承诺，也只是移动这个指针 | `project_state.rs:901-935` |

**结论：分裂只存在于前端。** 合并不需要改数据模型，主要是删掉前端的一层假象 + 修下面三个真 bug。

> 标注：以上为**读码所得**（已核对源文件）。下面 §2 的三条用户可见症状是**由代码推断**，未在运行的 app 里复现。

---

## 2. 读码翻出的三个真 bug

### B1 —— 换一次承诺，多一条永久冻结的「进行中」任务

`ReplaceCommitment`（`project_state.rs:1018-1043`）**不改旧 work item 的状态**：它以 `Doing` 留在 `work_items` 里。同时它出现在 commitment transition 里 → `mvp.rs:619-629` 的 `referenced` 集合包含它 → `linked_work_item_id` 非 null → 前端把它当"被承诺锁定"：

- 状态下拉 `disabled`（`TaskBoard.tsx:295`）
- 删除按钮 `disabled`（`TaskBoard.tsx:333`）
- 看板卡片显示「状态由当前承诺处置管理」（`TaskBoard.tsx:227`）

**换 N 次承诺 = 任务清单里 N 条改不动、删不掉的「进行中」。**

### B2 —— 完成过的承诺同样永久冻结

`CompleteCommitment` 把 item 置 `Done`（`project_state.rs:1003`），但它仍在 `referenced` 里 → 同样锁死：不能重开、不能删。

**根因同一个**：`linked_work_item_id` 的语义是「曾出现在任何一次 transition 里」，而前端读它的方式是「正被当前承诺占用」。两个语义不是一回事。

### 订正（实现 PR-1 时发现，方案初稿判断有误）

上面「解除 disable 即可」是**错的**。`project_state.rs:647-670` 有一层**防篡改校验**：凡被承诺碰过的 work item，其状态由 transition 日志推导（`replay_and_validate_commitment_history`），解析时逐条比对，不符即判定 `InvalidDocument`——测试里有 `parse_rejects_forged_status_after_undo_clear` 之类明确守着它。

所以前端的冻结不是意外，是在守护 core 不变量。正确修法是**同步改推导规则**：写入侧把 replace/clear 的旧条目置 `Planned`，推导侧 `apply_status_transition` / `apply_status_correction` 必须给出同样的期望值，否则文档会校验失败。PR-1 已按此实现。

### B3 —— 手写的承诺在同一屏出现两遍

`set_commitment` push 新 work item → 任务清单立刻渲染它（带「当前承诺」标签），而上方承诺面板显示同样的文字。

---

## 3. 挡在合并前面的一条核心规则（**需要你拍板**）

`project_state.rs:907-911`：

```rust
if work_item_is_referenced(state, &work_item_id) {
    return Err(ProjectStateError::InvalidCommand(
        "a historical commitment cannot be promoted again".into(),
    ));
}
```

**一条任务这辈子只能被设为承诺一次。**

在「两个实体」的旧模型里这说得通（承诺是一次性的仪式）。在「一份清单 + 星标」的新模型里它是用户敌意的：星标 → 改主意换掉 → 想换回来 → 被核心层拒绝，而界面上看不出为什么。

三个选项：

- **(a) 放宽规则**：允许重复星标，每次都推一条新的 `Set` transition。审计轨迹变长但仍然完整、仍然 append-only（符合 §7 ADR）。**我倾向这个。**
- **(b) 保留规则**：星标控件对历史条目置灰 + 给出可读理由。诚实，但每个用过一次的任务都带一块疤。
- **(c) 换到时才创建新条目**：即今天的 replace 行为。这正是 B1 的成因，不推荐。

---

## 4. 目标形态

```
任务清单  12
────────────────────────────────
★ 跑通 migration 的回滚路径      今天到期
  ─ 现在做这条 ·  完成   换一条
────────────────────────────────
  写 R2 验收清单                 3 天后
  ? 想清楚 focus strip 的取舍
  修 TaskBoard 的 autosave
```

规则：

1. **一个项目只有一份清单**。其中至多一条被标为「现在做这条」，置顶 + 星标。
2. **只有一条录入路径**：新增任务。星标是列表里的一个动作，不是另一个表单。
3. **星标不改变可编辑性**。星标任务的日期、标签、备注、文字照常可改。真正需要受限的只有"删除当前星标任务"，那给一句确认即可。
4. **完成 = 状态改成已完成**，顺手取消星标。不需要一个叫「完成」的独立按钮和一个叫「已完成」的状态并存。

---

## 5. 改动清单

### PR-1 `fix(record)`：解冻任务（先修 bug，不动概念）—— **已实现**

| 文件 | 改动 |
|---|---|
| `crates/omniproj-desktop/src/mvp.rs` | 删掉 `linked_work_item_id`（它只是 item 自己的 id，除布尔外无信息），换成 `was_committed: bool`，注释写明它只记历史、不得用于门控编辑 |
| `crates/omniproj-core/src/project_state.rs` | `ReplaceCommitment` / `ClearCommitment` 把旧 item 置 `Planned` 放回清单；`undo` 对应还原为 `Doing` |
| 同上（推导侧） | `apply_status_transition` / `apply_status_correction` 同步给出相同期望值，否则触发上面的防篡改校验 |
| `TaskBoard.tsx` | `locked` 改为 `is_current_commitment`；promote 按钮对 `was_committed` 的条目不再提供（core 会拒绝）；`promote()` 补上缺失的错误处理（原先 rejection 未捕获，点击看起来毫无反应） |

这一步**独立可发**，不依赖任何概念决策，而且是收益最高的一刀。

**本轮发现、明确推迟的一个问题**：`undo` 一次「把已有任务提升为承诺」的操作，会把该任务置 `Abandoned` → 它从清单里消失。修它需要让 transition 记录「本次 set 是否新建了 item」，因为推导侧只看 transition、无法区分两种 set。属于持久化格式变更，另议。

### PR-2 `refactor(record)`：合并承诺与任务（依赖 §3 的拍板）

| 文件 | 改动 |
|---|---|
| `components/projects/CurrentCommitment.tsx` | **删除**。214 行整个组件下线 |
| `components/projects/ProjectOverview.tsx:85` | 移除 `<CurrentCommitment>`；星标任务由 `TaskBoard` 置顶渲染 |
| `components/projects/TaskBoard.tsx` | 星标行加「现在做这条 / 完成 / 换一条」三个 inline 动作；「换一条」= 取消当前星标 + 星标另一条（不再创建新条目） |
| `project_state.rs:907-911` | 按 §3 的选择放宽或保留 |
| `i18n/I18nProvider.tsx:141-161` | `commitment.*` 大部分退役；保留成功/冲突提示 |

`CommitmentHistory`（审计轨迹）**保留**，它是宪法 §7 的地基，只是搬到「项目」tab。

### PR-3 `fix(desktop)`：文案与信息层级

| 改动 | 位置 |
|---|---|
| 删掉所有「人工 XX」kicker | `commitment.kicker` `plan.kicker` `task.kicker` `framing.kicker`（`I18nProvider.tsx:141, 230, 239, 164`）。作者边界（人 vs agent）是内部约束，不该当标签用；真要区分就给 agent 产物加小图标 |
| 「计划与决策日志」搬出 plan tab，独立成 tab | `ProjectOverview.tsx:116-122` 现在把 TaskBoard + PlanLog 并排放，两个同构列表挨着 = 用户无法判断该记哪边 |
| PlanLog 措辞改成人话 | 「记录决策」→「为什么这么做 / 为什么不做」 |
| **commit SHA 不许手抄** | `PlanLog.tsx:39` 的自由文本输入改成"从最近提交里选"。手抄 SHA 直接违反宪法 §5 原则 4「git 锚点自动，否则不做」 |
| 「更多操作」折叠删除 | `CurrentCommitment.tsx:175-185`。撤销改成跟在成功提示旁边的 inline 链接；清除并入「换一条」（留空即清除） |
| 首屏加一句定位 | charter §3 那句话在 app 里一个字都没有 |

### PR-4 `refactor(record)`：收敛任务视图

三个视图（列表 / 看板 / 按时间）能力不一致：list 行展开后可编日期/标签/备注/删除，board 和 time 的卡片只有一个状态下拉（`TaskBoard.tsx:214-232`）。

**收成两个（列表 + 按时间），或让三个能力一致。** 倾向前者——宪法 §6 明说「一个视图若不能先以一屏排序列表成立，就是过度设计」，看板没有通过这条。

同时 `TaskBoard.tsx` 已 339 行、一个 section 里塞了六层控件（新增行 / 视图切换 / 标签过滤 / Agent 提案 / 消息条 / 列表），拆分组件。

---

## 6. 明确不做

- 不动 `~/.omniproj` 的存储格式和迁移。
- 不动审计轨迹的 append-only 语义。
- 不新增任何 agent 能力——本轮纯粹是把已有的东西讲清楚。

---

## 7. 建议顺序

**PR-1 先行**（修 bug，无争议，收益最高）→ §3 拍板 → PR-2 → PR-3 → PR-4。

PR-1 落地后先真用几天再动 PR-2：如果解冻之后「两个实体」的痛感大幅下降，PR-2 的刀口还可以再收窄。
