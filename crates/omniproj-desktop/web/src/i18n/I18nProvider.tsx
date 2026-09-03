import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import type { AppError, ErrorCode } from "../domain/errors";
import type {
  CommitmentTransitionKind,
  ProjectStatus,
  ReviewReasonCode,
  WorkItemStatus,
} from "../domain/project";

export type Locale = "zh-CN" | "en";
export const LOCALE_STORAGE_KEY = "omniproj.locale";

const zh = {
  "language.label": "界面语言",
  "language.zh": "中文",
  "language.en": "English",
  "common.cancel": "取消",
  "common.retry": "重试",
  "common.copyText": "复制文本",
  "common.required": "必填",
  "common.tryAgain": "重试",
  "shell.hideSidebar": "隐藏侧边栏",
  "shell.showSidebar": "显示侧边栏",
  "shell.closeSidebar": "关闭侧边栏",
  "shell.backProjects": "返回项目列表",
  "shell.filterProjects": "筛选项目",
  "shell.searchProjects": "搜索项目",
  "shell.primaryNav": "主导航",
  "shell.projects": "项目",
  "shell.archived": "已归档",
  "shell.addProject": "添加项目",
  "shell.settings": "设置",
  "shell.newProject": "新建项目",
  "shell.refresh": "刷新",
  "shell.refreshing": "正在刷新项目",
  "shell.localReadonly": "本地运行 · 仅读取项目源",
  "shell.upToDate": "项目已是最新状态。",
  "shell.refreshStarted": "正在刷新项目…",
  "shell.refreshed": "项目已刷新。",
  "shell.refreshFailed": "有 {count} 个项目无法刷新，已保留上次获取的事实。",
  "index.workspace": "工作空间",
  "index.summary": "从当前承诺与实际工作状态重新进入每个项目。",
  "index.projectCount": "{count} 个项目", "index.projectCountUnit": "个项目",
  "index.loading": "正在加载项目…",
  "index.loadFailed": "无法加载项目。",
  "index.emptyTitle": "还没有项目",
  "index.emptyBody": "添加一个项目，开始重新进入并推进工作。",
  "index.addProject": "添加项目",
  "index.reviewOrderDetail": "审视顺序（先处理需要明确决策的项目）",
  "index.reviewInterval": "承诺审视周期：{days} 天",
  "index.reviewFilters": "审视筛选",
  "index.filterAll": "全部",
  "index.filterNeedsReview": "待审视",
  "index.filterWaiting": "等待中",
  "index.filterParked": "已搁置",
  "index.filterArchived": "已归档",
  "index.sort": "排序",
  "index.reviewOrder": "审视顺序",
  "index.sortName": "名称",
  "index.sortRecentCommit": "最近提交",
  "index.noMatch": "没有符合当前筛选条件的项目。",
  "index.moreFilters": "更多筛选与排序",
  "index.lifecycleFilters": "生命周期筛选",
  "index.needsDecision": "需要你决定",
  "index.otherProjects": "其他项目",
  "overview.loading": "正在加载项目…",
  "overview.loadFailed": "无法加载此项目。",
  "overview.title": "项目概览",
  "workspace.label": "项目视图",
  "workspace.reentry": "重新进入",
  "workspace.plan": "规划与任务",
  "workspace.activity": "查看实际变化",
  "workspace.project": "项目管理",
  "rail.label": "项目导航", "rail.search": "搜索项目", "rail.allProjects": "全部项目",
  "rail.noMatch": "没有匹配的项目", "rail.collapse": "收起项目栏", "rail.expand": "展开项目栏",
  "rail.resize": "调整项目栏宽度", "rail.needsDecision": "需要决定",
  "reentry.kicker": "重入上下文",
  "reentry.title": "方向与变化",
  "reentry.missingObjective": "尚未定义项目目标",
  "reentry.missingOutcome": "尚未定义期望结果",
  "reentry.sinceCommitment": "自当前承诺设定后",
  "reentry.delta": "观测到 {commits} 次提交，当前工作区有 {changed} 项变化",
  "reentry.noActual": "尚无可用的仓库观测",
  "reentry.latest": "最近变化",
  "settingsPage.kicker": "全局配置",
  "settingsPage.title": "设置",
  "settingsPage.description": "提醒与 Agent Provider 在此统一配置，不占用项目重入路径。",
  "notFound.eyebrow": "未知路径",
  "notFound.title": "找不到页面",
  "notFound.body": "页面位置可能已更改，但你的项目和本地状态没有受到影响。",
  "notFound.back": "返回项目列表",
  "row.noCommitment": "暂无当前承诺",
  "row.notObserved": "尚未观测",
  "row.noCommits": "暂无提交",
  "row.noReview": "当前无需审视",
  "row.commitment": "承诺：{text}",
  "row.observed": "已观测 {head}",
  "row.review": "审视：{label}{more}",
  "row.more": "，另有 {count} 项",
  "row.lastActivity": "最近提交 {time}",
  "row.silentDays": "已静默 {days} 天",
  "row.changed": "{count} 项变更",
  "row.clean": "工作区干净",
  "row.commitsSince": "此后 {count} 次提交",
  "activity.summary": "最近 16 周共 {total} 次提交",
  "activity.unknown": "静默时间未知",
  "activity.silenceWithThreshold": "静默 {days} 天 · 提醒阈值 {threshold} 天",
  "head.detached": "游离 HEAD",
  "head.unborn": "尚无提交",
  "head.onBranch": "位于 {branch}",
  "head.branchUnborn": "{branch}（尚无提交）",
  "review.noneKicker": "审视状态",
  "review.noneTitle": "当前无需审视",
  "review.noneBody": "此项目目前没有由确定性规则产生的审视信号。",
  "review.kicker": "需要关注",
  "review.title": "审视原因",
  "review.moreOne": "另有 1 项审视原因：{labels}",
  "review.moreMany": "另有 {count} 项审视原因：{labels}",
  "observed.kicker": "仓库事实",
  "observed.title": "实际观测",
  "observed.sourceNoHistory": "当前无法读取项目源，且没有可显示的历史观测。",
  "observed.notYet": "尚未观测。",
  "observed.stale": "项目源当前不可用，正在显示上次成功观测{time}。",
  "observed.fromTime": "（{time}）",
  "observed.head": "HEAD",
  "observed.lastCommit": "最近提交",
  "observed.workingTree": "工作区",
  "observed.workingTreeValue": "变更 {changed}，已暂存 {staged}，未跟踪 {untracked}",
  "observed.sinceCommitment": "自本次承诺以来",
  "observed.commitsSince": "设定承诺后观测到 {count} 次仓库提交",
  "observed.observedAt": "观测时间",
  "commitment.kicker": "人工承诺",
  "commitment.title": "当前下一步",
  "commitment.confirm": "保持这一步",
  "commitment.complete": "完成",
  "commitment.replace": "替换",
  "commitment.clear": "清除",
  "commitment.new": "新承诺",
  "commitment.reason": "原因",
  "commitment.replaceReason": "替换原因",
  "commitment.saveReplacement": "保存替换",
  "commitment.save": "保存承诺",
  "commitment.undo": "撤销上次更改",
  "commitment.moreActions": "更多操作",
  "commitment.auditFailed": "状态已保存，但审计提交失败。更改已持久化，无需重新提交。",
  "commitment.conflict": "你操作期间此项目已发生变化。已加载最新状态；请审视后重新提交，你输入的文本仍被保留。",
  "commitment.setSuccess": "承诺已设定。",
  "commitment.confirmSuccess": "承诺已确认。",
  "commitment.completeSuccess": "承诺已完成。",
  "commitment.replaceSuccess": "承诺已替换。",
  "commitment.clearSuccess": "承诺已清除。",
  "commitment.undoSuccess": "已撤销上次更改。",
  "history.kicker": "审计轨迹",
  "history.title": "最近承诺历史",
  "framing.kicker": "人工定义的意图",
  "framing.setupTitle": "完成设置",
  "framing.title": "项目定义",
  "framing.setupIntro": "在将项目转入进行状态前，请定义预期结果和第一项具体承诺。",
  "framing.objective": "目标",
  "framing.desiredOutcome": "期望结果",
  "framing.phase": "阶段",
  "framing.optional": "可选",
  "framing.firstCommitment": "第一项承诺",
  "framing.save": "保存项目定义",
  "framing.setupSuccess": "设置已完成。",
  "framing.saveSuccess": "项目定义已保存。",
  "framing.conflict": "你操作期间此项目已发生变化。已加载最新状态；请审视后重新提交，你输入的文本仍被保留。",
  "lifecycle.kicker": "项目状态",
  "lifecycle.title": "生命周期",
  "lifecycle.setStatus": "设置状态",
  "lifecycle.reason": "原因",
  "lifecycle.statusReason": "状态原因",
  "lifecycle.reviewDate": "审视日期",
  "lifecycle.reviewDateOptional": "审视日期（可选）",
  "lifecycle.confirmArchive": "确认归档",
  "lifecycle.archiveNotice": "我了解归档后，此项目将不再出现在默认工作视图中。",
  "lifecycle.update": "更新状态",
  "lifecycle.success": "项目状态已更新。",
  "recovery.kicker": "需要恢复",
  "recovery.title": "项目源不可用",
  "recovery.description": "此项目的源状态为“{status}”。请将它指向仓库的新位置以恢复观测；项目身份和历史不会改变。",
  "recovery.choose": "选择新位置…",
  "recovery.duplicate": "该文件夹已注册为“{name}”。",
  "recovery.invalid": "无法使用该文件夹（{state}）。",
  "recovery.confirm": "确认重新关联",
  "recovery.confirmNotice": "我确认这是同一项目的仓库。",
  "recovery.relink": "重新关联项目源",
  "recovery.success": "项目源已重新关联。",
  "mutation.auditFailed": "状态已保存，但审计提交失败。",
  "mutation.conflict": "你操作期间此项目已发生变化。请审视最新状态后重新提交。",
  "add.title": "添加项目",
  "add.kicker": "本地 Git 仓库",
  "add.description": "注册仓库，但不会更改其中的任何内容。",
  "add.close": "关闭添加项目窗口",
  "add.choose": "选择目录",
  "add.chooseAgain": "重新选择目录",
  "add.chooseProject": "选择项目目录",
  "add.chooseDifferent": "选择其他目录",
  "add.readonly": "OmniProj 仅读取 Git 事实，并保持仓库本身不被修改。",
  "add.ready": "仓库已就绪",
  "add.noCommits": "暂无提交",
  "add.projectName": "项目名称",
  "add.duplicate": "已注册为“{name}”。",
  "add.openExisting": "打开已有项目",
  "add.register": "注册",
  "add.registered": "项目已注册。",
  "add.missing": "该文件夹已不存在。",
  "add.unreadable": "无法读取该文件夹。请检查权限后重试。",
  "add.notGit": "该文件夹不是 Git 仓库。",
  "add.bare": "暂不支持裸 Git 仓库。",
  "add.observationFailed": "无法读取仓库：{message}",
  "add.validateFailed": "无法验证该文件夹。",
  "add.registerFailed": "无法注册该项目。",
  "time.justNow": "刚刚",
  "time.minuteAgo": "1 分钟前",
  "time.minutesAgo": "{count} 分钟前",
  "time.hourAgo": "1 小时前",
  "time.hoursAgo": "{count} 小时前",
  "time.yesterday": "昨天",
  "time.daysAgo": "{count} 天前",
  "task.kicker": "人工任务", "task.title": "任务清单", "task.new": "新增任务", "task.unclear": "未成形（?）", "task.add": "添加任务", "task.addDisabled": "先输入任务内容", "task.loading": "正在加载任务…", "task.empty": "还没有任务。先记录一个具体的下一步。", "task.status": "任务状态", "task.due": "预期完成日期", "task.advance": "让 Agent 拆解", "task.advanceReady": "Agent 已生成候选子任务，请选择要采纳的条目。", "task.remove": "删除",
  "task.open": "待处理", "task.doing": "进行中", "task.done": "已完成", "task.note": "问题备注", "task.save": "保存任务", "task.relationship": "一份清单。其中至多一条被标为「现在做这条」。", "task.makeCommitment": "设为现在做这条", "task.currentCommitment": "现在做这条",
  "task.nowDoingEmpty": "还没有选定现在做哪一条。", "task.complete": "完成", "task.switchAway": "换一条", "task.stillThis": "仍然是这一步", "task.undo": "撤销上次更改", "task.switchAwayHint": "这一步会回到清单，你可以再标另一条。", "task.proposal": "Advance 候选", "task.adoptSelected": "采纳所选", "task.fromProposal": "采纳自 Agent 提案 {id}", "task.conflict": "任务文件已发生变化，已重新加载；请审视后再次保存。", "task.advancing": "Agent 正在生成候选…", "task.advanceFailed": "Agent 拆解失败，请检查 Agent 设置。",
  "task.tags": "标签", "task.tagsHint": "逗号分隔，最多 8 个", "task.filterTags": "按标签过滤", "task.tagFilterClear": "清除标签过滤", "task.notePlaceholder": "记下卡在哪里", "task.autosave": "离开输入框即自动保存", "date.today": "今天", "date.tomorrow": "明天", "date.friday": "本周五", "date.nextMonday": "下周一", "date.clear": "清除日期", "tags.placeholder": "输入或选择标签", "tags.remove": "移除标签 {tag}", "tags.suggestions": "本项目已用标签", "tags.full": "最多 {max} 个标签",
  "board.viewLabel": "任务视图", "board.viewList": "列表", "board.viewBoard": "看板", "board.viewTime": "按时间", "board.moveTo": "移动到",
  "time.overdue": "逾期", "time.today": "今天", "time.thisWeek": "本周", "time.nextWeek": "下周", "time.later": "以后", "time.unscheduled": "未排期", "time.empty": "没有待到期的任务。",
  "focus.title": "今日聚焦", "focus.summary": "{projects} 个项目共 {items} 条任务逾期或今日到期", "board.locked": "状态由当前承诺处置管理", "board.overdue": "逾期 {days} 天", "board.dueSoon": "{days} 天后到期", "board.dueToday": "今天到期", "board.doneShowAll": "显示全部（{count}）", "board.doneCollapse": "收起", "board.columnEmpty": "暂无",
  "timeline.kicker": "Git 实际", "timeline.title": "提交时间线", "timeline.loading": "正在加载提交…", "timeline.empty": "暂无可显示的提交。", "timeline.attributed": "已归属任务：{ids}",
  "timeline.assign": "归属提交 {sha}", "timeline.assignNone": "选择归属任务",
  "attention.count": "待关注项目：{count}",
  "plan.kicker": "人工决策", "plan.title": "计划与决策日志", "plan.newTitle": "新增决策标题", "plan.body": "决策依据（可选）", "plan.add": "记录决策", "plan.addDisabled": "先填写决策标题", "plan.loading": "正在加载决策…", "plan.empty": "还没有决策记录。", "plan.status": "决策状态", "plan.planned": "已计划", "plan.doing": "进行中", "plan.done": "已完成", "plan.abandoned": "已放弃", "plan.commit": "关联 commit SHA", "plan.saveCommit": "保存关联",
  "settings.kicker": "提醒设置", "settings.title": "提醒", "settings.enabled": "启用提醒", "settings.cadence": "提醒频率", "settings.daily": "每天", "settings.off": "关闭", "settings.threshold": "静默阈值（天）", "settings.save": "保存设置", "settings.test": "发送测试提醒", "settings.saved": "提醒设置已保存。", "settings.tested": "测试提醒已发送。",
  "agent.kicker": "Agent 设置", "agent.title": "拆解模型", "agent.ready": "可用", "agent.notReady": "未就绪", "agent.privacy": "远程 Advance 只发送任务文本和问题备注；API key 保存在系统钥匙串，不写入 OmniProj 数据目录。", "agent.provider": "Provider（服务商）", "agent.model": "模型", "agent.apiKey": "API key", "agent.keyStored": "已保存于系统钥匙串；留空保持不变", "agent.keyRequired": "需要 API key", "agent.local": "本地", "agent.consent": "我同意把任务文本和问题备注发送给所选远程 provider。", "agent.save": "保存 Agent 设置", "agent.test": "测试连接", "agent.testDisabled": "先保存 Agent 设置并确认可用，才能测试连接", "agent.saveDisabled": "请先填写模型名称", "agent.testing": "正在测试…", "agent.saved": "Agent 设置已保存。", "agent.tested": "Agent 连接正常。", "agent.saveFailed": "无法保存 Agent 设置。", "agent.testFailed": "Agent 连接测试失败。",
  "dogfood.kicker": "产品验证", "dogfood.title": "重入计时", "dogfood.summary": "已记录 {events}/20 次，覆盖 {projects}/5 个项目；中位重入时间 {median} 秒。", "dogfood.start": "开始重入", "dogfood.ready": "已明确下一步，准备工作", "dogfood.recorded": "本次重入已记录。",
  "graph.kicker": "Git 实际", "graph.title": "提交拓扑摘要", "graph.loading": "正在加载提交拓扑…", "graph.empty": "暂无可显示的 Git 提交拓扑。",
} as const;

type MessageKey = keyof typeof zh;
type Params = Record<string, string | number>;

const en: Record<MessageKey, string> = {
  "language.label": "Interface language", "language.zh": "中文", "language.en": "English",
  "common.cancel": "Cancel", "common.retry": "Retry", "common.copyText": "Copy text", "common.required": "Required", "common.tryAgain": "Try again",
  "shell.hideSidebar": "Hide sidebar", "shell.showSidebar": "Show sidebar", "shell.closeSidebar": "Close sidebar", "shell.backProjects": "Back to projects", "shell.filterProjects": "Filter projects", "shell.searchProjects": "Search projects", "shell.primaryNav": "Primary", "shell.projects": "Projects", "shell.archived": "Archived", "shell.addProject": "Add Project", "shell.settings": "Settings", "shell.newProject": "New project", "shell.refresh": "Refresh", "shell.refreshing": "Refreshing projects", "shell.localReadonly": "Local · read-only sources", "shell.upToDate": "Projects are up to date.", "shell.refreshStarted": "Refreshing projects…", "shell.refreshed": "Projects refreshed.", "shell.refreshFailed": "{count} project(s) could not be refreshed. Last known facts were preserved.",
  "index.workspace": "Workspace", "index.summary": "Re-enter each project through its current commitment and observed work.", "index.projectCount": "{count} project(s)", "index.projectCountUnit": "projects", "index.loading": "Loading projects…", "index.loadFailed": "Couldn't load projects.", "index.emptyTitle": "No projects yet", "index.emptyBody": "Add a project to begin re-entering and advancing your work.", "index.addProject": "Add project", "index.reviewOrderDetail": "Review order (projects needing an explicit decision first)", "index.reviewInterval": "Commitment review interval: {days} days", "index.reviewFilters": "Review filters", "index.filterAll": "All", "index.filterNeedsReview": "Needs review", "index.filterWaiting": "Waiting", "index.filterParked": "Parked", "index.filterArchived": "Archived", "index.sort": "Sort", "index.reviewOrder": "Review order", "index.sortName": "Name", "index.sortRecentCommit": "Recent commit", "index.noMatch": "No projects match this filter.", "index.moreFilters": "More filters and sorting", "index.lifecycleFilters": "Lifecycle filters", "index.needsDecision": "Needs your decision", "index.otherProjects": "Other projects",
  "overview.loading": "Loading project…", "overview.loadFailed": "Couldn't load this project.", "overview.title": "Project overview", "workspace.label": "Project views", "workspace.reentry": "Re-enter", "workspace.plan": "Planning and tasks", "workspace.activity": "View observed change", "workspace.project": "Project management", "rail.label": "Project navigation", "rail.search": "Search projects", "rail.allProjects": "All projects", "rail.noMatch": "No matching project", "rail.collapse": "Collapse project rail", "rail.expand": "Expand project rail", "rail.resize": "Resize project rail", "rail.needsDecision": "Needs a decision", "reentry.kicker": "Re-entry context", "reentry.title": "Direction and change", "reentry.missingObjective": "Project objective is not defined", "reentry.missingOutcome": "Desired outcome is not defined", "reentry.sinceCommitment": "Since the current commitment was set", "reentry.delta": "{commits} commit(s) observed; {changed} working-tree change(s) now", "reentry.noActual": "No repository observation is available", "reentry.latest": "Latest change", "settingsPage.kicker": "Global configuration", "settingsPage.title": "Settings", "settingsPage.description": "Configure language, reminders, and the Agent provider outside the project re-entry path.",
  "notFound.eyebrow": "Unknown route", "notFound.title": "Page not found", "notFound.body": "The page may have moved, but your projects and local state are unchanged.", "notFound.back": "Back to Projects",
  "row.noCommitment": "No current commitment", "row.notObserved": "Not yet observed", "row.noCommits": "no commits", "row.noReview": "No review needed", "row.commitment": "Commitment: {text}", "row.observed": "Observed {head}", "row.review": "Review: {label}{more}", "row.more": ", +{count} more", "row.changed": "{count} changed", "row.clean": "clean", "row.commitsSince": "{count} commit(s) since", "row.lastActivity": "Last commit {time}", "row.silentDays": "Silent for {days} days", "activity.summary": "{total} commits in the last 16 weeks", "activity.unknown": "Silence unknown", "activity.silenceWithThreshold": "Silent {days} days · reminder threshold {threshold} days", "head.detached": "Detached HEAD", "head.unborn": "Unborn (no commits yet)", "head.onBranch": "On {branch}", "head.branchUnborn": "{branch} (unborn, no commits yet)",
  "review.noneKicker": "Review state", "review.noneTitle": "No review needed", "review.noneBody": "This project has no deterministic review signal right now.", "review.kicker": "Needs attention", "review.title": "Review reasons", "review.moreOne": "1 more review reason: {labels}", "review.moreMany": "{count} more review reasons: {labels}",
  "observed.kicker": "Repository facts", "observed.title": "Observed actual", "observed.sourceNoHistory": "The source could not be read; there is no earlier observation to show.", "observed.notYet": "Not yet observed.", "observed.stale": "Source currently unavailable — showing the last successful observation{time}.", "observed.fromTime": " from {time}", "observed.head": "Head", "observed.lastCommit": "Last commit", "observed.workingTree": "Working tree", "observed.workingTreeValue": "{changed} changed, {staged} staged, {untracked} untracked", "observed.sinceCommitment": "Since this commitment", "observed.commitsSince": "{count} repository commit(s) observed since it was set", "observed.observedAt": "Observed",
  "commitment.kicker": "Human commitment", "commitment.title": "Current next step", "commitment.confirm": "Keep this step", "commitment.complete": "Complete", "commitment.replace": "Replace", "commitment.clear": "Clear", "commitment.new": "New commitment", "commitment.reason": "Reason", "commitment.replaceReason": "Replace reason", "commitment.saveReplacement": "Save replacement", "commitment.save": "Save commitment", "commitment.undo": "Undo last change", "commitment.moreActions": "More actions", "commitment.auditFailed": "State saved; the audit commit failed. Your change is durable — no need to resend.", "commitment.conflict": "This project changed since you started. The latest state is loaded; review it and resubmit — your text is kept.", "commitment.setSuccess": "Commitment set.", "commitment.confirmSuccess": "Commitment confirmed.", "commitment.completeSuccess": "Commitment completed.", "commitment.replaceSuccess": "Commitment replaced.", "commitment.clearSuccess": "Commitment cleared.", "commitment.undoSuccess": "Last change undone.",
  "history.kicker": "Audit trail", "history.title": "Recent commitment history", "mutation.auditFailed": "State saved; audit commit failed.", "mutation.conflict": "This project changed since you started. Review the latest and resubmit.",
  "framing.kicker": "Human-authored intent", "framing.setupTitle": "Complete setup", "framing.title": "Project framing", "framing.setupIntro": "Define the outcome and first concrete commitment before moving this project into active work.", "framing.objective": "Objective", "framing.desiredOutcome": "Desired outcome", "framing.phase": "Phase", "framing.optional": "Optional", "framing.firstCommitment": "First commitment", "framing.save": "Save framing", "framing.setupSuccess": "Setup complete.", "framing.saveSuccess": "Framing saved.", "framing.conflict": "This project changed since you started. The latest state is loaded; review and resubmit — your text is kept.",
  "lifecycle.kicker": "Project state", "lifecycle.title": "Lifecycle", "lifecycle.setStatus": "Set status", "lifecycle.reason": "Reason", "lifecycle.statusReason": "Status reason", "lifecycle.reviewDate": "Review date", "lifecycle.reviewDateOptional": "Review date (optional)", "lifecycle.confirmArchive": "Confirm archive", "lifecycle.archiveNotice": "I understand archiving removes this project from the default operating view.", "lifecycle.update": "Update status", "lifecycle.success": "Project status updated.",
  "recovery.kicker": "Recovery required", "recovery.title": "Source unavailable", "recovery.description": "This project's source is {status}. Point it at the repository's new location to restore observations — the project keeps its identity and history.", "recovery.choose": "Choose new location…", "recovery.duplicate": "That folder is already registered as “{name}”.", "recovery.invalid": "That folder can't be used ({state}).", "recovery.confirm": "Confirm relink", "recovery.confirmNotice": "I confirm this is the same project's repository.", "recovery.relink": "Relink source", "recovery.success": "Source relinked.",
  "add.title": "Add Project", "add.kicker": "Local Git repository", "add.description": "Register a repository without changing anything inside it.", "add.close": "Close Add Project", "add.choose": "Choose directory", "add.chooseAgain": "Choose directory again", "add.chooseProject": "Choose project directory", "add.chooseDifferent": "Choose a different directory", "add.readonly": "OmniProj reads Git facts and keeps the repository itself read-only.", "add.ready": "Repository ready", "add.noCommits": "No commits yet", "add.projectName": "Project name", "add.duplicate": "Already registered as “{name}”.", "add.openExisting": "Open existing project", "add.register": "Register", "add.registered": "Project registered.", "add.missing": "That folder no longer exists.", "add.unreadable": "That folder can't be read. Check permissions and try again.", "add.notGit": "That folder isn't a Git repository.", "add.bare": "Bare Git repositories aren't supported.", "add.observationFailed": "Couldn't read the repository: {message}", "add.validateFailed": "Couldn't validate that folder.", "add.registerFailed": "Couldn't register that project.",
  "time.justNow": "just now", "time.minuteAgo": "1 minute ago", "time.minutesAgo": "{count} minutes ago", "time.hourAgo": "1 hour ago", "time.hoursAgo": "{count} hours ago", "time.yesterday": "yesterday", "time.daysAgo": "{count} days ago",
  "task.kicker": "Human tasks", "task.title": "Task list", "task.new": "New task", "task.unclear": "Not yet clear (?)", "task.add": "Add task", "task.addDisabled": "Enter the task text first", "task.loading": "Loading tasks…", "task.empty": "No tasks yet. Record one concrete next action.", "task.status": "Task status", "task.due": "Expected completion date", "task.advance": "Ask Agent to break down", "task.advanceReady": "The Agent generated candidate subtasks. Select the ones to adopt.", "task.remove": "Remove",
  "task.open": "Open", "task.doing": "Doing", "task.done": "Done", "task.note": "Problem note", "task.save": "Save task", "task.relationship": "One list. At most one item is marked as the one you are doing now.", "task.makeCommitment": "Do this one now", "task.currentCommitment": "Doing this now",
  "task.nowDoingEmpty": "No step picked yet.", "task.complete": "Complete", "task.switchAway": "Switch away", "task.stillThis": "Still this one", "task.undo": "Undo the last change", "task.switchAwayHint": "This step returns to the list; you can mark another one.", "task.proposal": "Advance candidates", "task.adoptSelected": "Adopt selected", "task.fromProposal": "Adopted from Agent proposal {id}", "task.conflict": "The task file changed and was reloaded. Review it before saving again.", "task.advancing": "The Agent is generating candidates…", "task.advanceFailed": "Agent breakdown failed. Check Agent settings.",
  "task.tags": "Tags", "task.tagsHint": "Comma-separated, up to 8", "task.filterTags": "Filter by tag", "task.tagFilterClear": "Clear tag filter", "task.notePlaceholder": "Note what is blocking this", "task.autosave": "Saved when you leave the field", "date.today": "Today", "date.tomorrow": "Tomorrow", "date.friday": "This Friday", "date.nextMonday": "Next Monday", "date.clear": "Clear date", "tags.placeholder": "Type or pick a tag", "tags.remove": "Remove tag {tag}", "tags.suggestions": "Tags used in this project", "tags.full": "Up to {max} tags",
  "board.viewLabel": "Task view", "board.viewList": "List", "board.viewBoard": "Board", "board.viewTime": "By time", "board.moveTo": "Move to",
  "time.overdue": "Overdue", "time.today": "Today", "time.thisWeek": "This week", "time.nextWeek": "Next week", "time.later": "Later", "time.unscheduled": "Unscheduled", "time.empty": "No upcoming tasks.",
  "focus.title": "Today's focus", "focus.summary": "{items} task(s) overdue or due today across {projects} project(s)", "board.locked": "Status is managed by commitment actions", "board.overdue": "Overdue {days}d", "board.dueSoon": "Due in {days}d", "board.dueToday": "Due today", "board.doneShowAll": "Show all ({count})", "board.doneCollapse": "Collapse", "board.columnEmpty": "None",
  "timeline.kicker": "Git actual", "timeline.title": "Commit timeline", "timeline.loading": "Loading commits…", "timeline.empty": "No commits to show.", "timeline.attributed": "Attributed tasks: {ids}",
  "timeline.assign": "Attribute commit {sha}", "timeline.assignNone": "Choose a task",
  "attention.count": "Projects needing attention: {count}",
  "plan.kicker": "Human decisions", "plan.title": "Plan & decision log", "plan.newTitle": "New decision title", "plan.body": "Rationale (optional)", "plan.add": "Record decision", "plan.addDisabled": "Enter a decision title first", "plan.loading": "Loading decisions…", "plan.empty": "No decisions recorded yet.", "plan.status": "Decision status", "plan.planned": "Planned", "plan.doing": "Doing", "plan.done": "Done", "plan.abandoned": "Abandoned", "plan.commit": "Linked commit SHA", "plan.saveCommit": "Save link",
  "settings.kicker": "Reminder settings", "settings.title": "Reminders", "settings.enabled": "Enable reminders", "settings.cadence": "Reminder cadence", "settings.daily": "Daily", "settings.off": "Off", "settings.threshold": "Silence threshold (days)", "settings.save": "Save settings", "settings.test": "Send test reminder", "settings.saved": "Reminder settings saved.", "settings.tested": "Test reminder sent.",
  "agent.kicker": "Agent settings", "agent.title": "Breakdown model", "agent.ready": "Ready", "agent.notReady": "Not ready", "agent.privacy": "Remote Advance sends only the task text and problem note. API keys stay in the system credential store and are never written to OmniProj data.", "agent.provider": "Provider", "agent.model": "Model", "agent.apiKey": "API key", "agent.keyStored": "Stored in system keychain; leave blank to keep it", "agent.keyRequired": "API key required", "agent.local": "local", "agent.consent": "I agree to send task text and problem notes to the selected remote provider.", "agent.save": "Save Agent settings", "agent.test": "Test connection", "agent.testDisabled": "Save the Agent settings and make them ready before testing", "agent.saveDisabled": "Enter a model name first", "agent.testing": "Testing…", "agent.saved": "Agent settings saved.", "agent.tested": "Agent connection is ready.", "agent.saveFailed": "Couldn't save Agent settings.", "agent.testFailed": "Agent connection test failed.",
  "dogfood.kicker": "Product validation", "dogfood.title": "Re-entry timer", "dogfood.summary": "Recorded {events}/20 events across {projects}/5 projects; median re-entry time {median} seconds.", "dogfood.start": "Start re-entry", "dogfood.ready": "Next action is clear; start work", "dogfood.recorded": "This re-entry was recorded.",
  "graph.kicker": "Git actual", "graph.title": "Commit topology summary", "graph.loading": "Loading commit topology…", "graph.empty": "No Git commit topology to display.",
};

export type Translate = (key: MessageKey, params?: Params) => string;

function interpolate(value: string, params?: Params): string {
  if (!params) return value;
  return value.replace(/\{(\w+)\}/g, (_, key: string) => String(params[key] ?? `{${key}}`));
}

function readStoredLocale(): Locale {
  if (typeof window === "undefined") return "zh-CN";
  try {
    const stored = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    return stored === "en" || stored === "zh-CN" ? stored : "zh-CN";
  } catch {
    return "zh-CN";
  }
}

interface I18nValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: Translate;
}

// Isolated leaf-component tests may render without the application provider; the production
// App always installs I18nProvider, whose no-preference default is Simplified Chinese.
const defaultTranslate: Translate = (key, params) => interpolate(en[key], params);
const I18nContext = createContext<I18nValue>({ locale: "en", setLocale: () => {}, t: defaultTranslate });

export function I18nProvider({ children, initialLocale }: { children: ReactNode; initialLocale?: Locale }) {
  const [locale, setLocaleState] = useState<Locale>(() => initialLocale ?? readStoredLocale());
  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      if (typeof window !== "undefined") window.localStorage.setItem(LOCALE_STORAGE_KEY, next);
    } catch {
      // The active session still switches language when storage is unavailable.
    }
  }, []);
  useEffect(() => {
    if (typeof document !== "undefined") document.documentElement.lang = locale;
  }, [locale]);
  const t = useCallback<Translate>((key, params) => interpolate((locale === "zh-CN" ? zh : en)[key], params), [locale]);
  const value = useMemo(() => ({ locale, setLocale, t }), [locale, setLocale, t]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  return useContext(I18nContext);
}

const PROJECT_STATUS_KEYS: Record<ProjectStatus, string> = {
  setup: "设置中", active: "进行中", waiting: "等待中", parked: "已搁置", archived: "已归档",
};
const PROJECT_STATUS_EN: Record<ProjectStatus, string> = {
  setup: "Setup", active: "Active", waiting: "Waiting", parked: "Parked", archived: "Archived",
};
const WORK_STATUS_KEYS: Record<WorkItemStatus, string> = {
  planned: "已计划", doing: "进行中", blocked: "受阻", done: "已完成", abandoned: "已放弃",
};
const WORK_STATUS_EN: Record<WorkItemStatus, string> = {
  planned: "Planned", doing: "Doing", blocked: "Blocked", done: "Done", abandoned: "Abandoned",
};
const REVIEW_REASON_ZH: Record<ReviewReasonCode, string> = {
  source_unavailable: "项目源不可用", complete_setup: "完成设置", needs_commitment: "需要承诺", overdue_work: "任务逾期", review_action: "审视实际进展", scheduled_review: "定期审视",
};
const REVIEW_REASON_EN: Record<ReviewReasonCode, string> = {
  source_unavailable: "Source unavailable", complete_setup: "Complete setup", needs_commitment: "Needs commitment", overdue_work: "Overdue work", review_action: "Review action", scheduled_review: "Scheduled review",
};
const TRANSITION_ZH: Record<CommitmentTransitionKind, string> = {
  set: "设定", confirmed: "确认", completed: "完成", replaced: "替换", cleared: "清除", correction: "纠正",
};
const TRANSITION_EN: Record<CommitmentTransitionKind, string> = {
  set: "Set", confirmed: "Confirmed", completed: "Completed", replaced: "Replaced", cleared: "Cleared", correction: "Correction",
};

export const projectStatusLabel = (status: ProjectStatus, locale: Locale) => locale === "zh-CN" ? PROJECT_STATUS_KEYS[status] : PROJECT_STATUS_EN[status];
export const workItemStatusLabel = (status: WorkItemStatus, locale: Locale) => locale === "zh-CN" ? WORK_STATUS_KEYS[status] : WORK_STATUS_EN[status];
export const reviewReasonLabel = (code: ReviewReasonCode, locale: Locale) => locale === "zh-CN" ? REVIEW_REASON_ZH[code] : REVIEW_REASON_EN[code];
export const transitionLabel = (kind: CommitmentTransitionKind, locale: Locale) => locale === "zh-CN" ? TRANSITION_ZH[kind] : TRANSITION_EN[kind];

const ERROR_ZH: Record<ErrorCode | "unknown", string> = {
  project_not_found: "找不到该项目。", invalid_input: "输入内容无效，请检查后重试。", invalid_path: "项目路径无效。",
  source_missing: "项目源已不存在。", source_unreadable: "无法读取项目源，请检查权限。", not_git_repository: "所选目录不是 Git 仓库。", bare_repository: "暂不支持裸 Git 仓库。", duplicate_source: "该项目源已经注册。", source_observation_failed: "无法读取仓库事实。",
  store_read_failed: "无法读取本地数据。", store_write_failed: "无法保存本地数据。", audit_commit_failed: "状态已保存，但审计提交失败。", revision_conflict: "项目已发生变化，请审视最新状态后重试。", current_commitment_exists: "当前已存在承诺。", no_current_commitment: "当前没有可操作的承诺。", current_commitment_changed: "当前承诺已发生变化。", reason_required: "必须填写原因。", transition_not_found: "找不到该变更记录。", undo_not_available: "当前没有可撤销的更改。", undo_conflict: "无法撤销，因为项目状态已发生变化。", unknown: "出现问题，请重试。",
};

export function localizeError(error: AppError, locale: Locale): string {
  return locale === "zh-CN" ? ERROR_ZH[error.code] : error.message;
}

export function localizeEvidence(line: string, locale: Locale): string {
  if (locale === "en") return line;
  const exact: Record<string, string> = {
    "missing objective": "缺少项目目标",
    "missing desired outcome": "缺少期望结果",
    "missing first commitment": "缺少第一项承诺",
    "no effective commitment transition recorded": "尚未记录有效的承诺变更",
  };
  if (exact[line]) return exact[line];
  const sourceStatus = line.match(/^source status: (available|moved|unreadable|missing)$/);
  if (sourceStatus) {
    const labels: Record<string, string> = { available: "可用", moved: "已移动", unreadable: "不可读", missing: "缺失" };
    return `项目源状态：${labels[sourceStatus[1]]}`;
  }
  const transition = line.match(/^last effective commitment transition: (set|confirmed|completed|replaced|cleared|correction) at (.+)$/);
  if (transition) {
    const labels: Record<string, string> = { set: "设定", confirmed: "确认", completed: "完成", replaced: "替换", cleared: "清除", correction: "纠正" };
    return `最近有效承诺变更：${labels[transition[1]]}于 ${transition[2]}`;
  }
  const interval = line.match(/^review interval: (\d+) days$/);
  if (interval) return `审视周期：${interval[1]} 天`;
  const overdueCount = line.match(/^overdue items: (\d+)$/);
  if (overdueCount) return `逾期任务：${overdueCount[1]} 项`;
  const overdueItem = line.match(/^due (\d{4}-\d{2}-\d{2}) \((\d+) days? overdue\): (.*)$/);
  if (overdueItem) return `预期 ${overdueItem[1]}，已逾期 ${overdueItem[2]} 天：${overdueItem[3]}`;
  const overdueMore = line.match(/^and (\d+) more overdue items?$/);
  if (overdueMore) return `…另有 ${overdueMore[1]} 项逾期`;
  const prefixes: Array<[string, string]> = [
    ["source status: ", "项目源状态："], ["last successful refresh: ", "最近成功刷新："],
    ["source error category: ", "项目源错误类别："], ["last effective commitment transition: ", "最近有效承诺变更："],
    ["review interval: ", "审视周期："], ["commitment set at: ", "承诺设定时间："],
    ["last effective set/confirmation: ", "最近有效设定/确认："], ["current commitment: ", "当前承诺："],
    ["status reason: ", "状态原因："], ["review date: ", "审视日期："],
  ];
  const prefix = prefixes.find(([source]) => line.startsWith(source));
  if (!prefix) return line;
  const value = line.slice(prefix[0].length);
  return prefix[1] + (value === "none recorded" ? "暂无记录" : value);
}
