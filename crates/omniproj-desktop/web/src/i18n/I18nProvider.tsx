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
  "common.tryAgain": "重试",
  "shell.backProjects": "返回项目列表",
  "shell.filterProjects": "筛选项目",
  "shell.searchProjects": "搜索项目",
  "shell.projects": "项目",
  "shell.settings": "设置",
  "shell.newProject": "新建项目",
  "shell.refresh": "刷新",
  "shell.refreshing": "正在刷新项目",
  "shell.upToDate": "项目已是最新状态。",
  "shell.refreshStarted": "正在刷新项目…",
  "shell.refreshed": "项目已刷新。",
  "shell.refreshFailed": "有 {count} 个项目无法刷新，已保留上次获取的事实。",
  "index.workspace": "项目",
  "index.summary": "每个项目一页清单，配上它的 git 活动。",
  "index.projectCount": "{count} 个项目", "index.projectCountUnit": "个项目",
  "index.loading": "正在加载项目…",
  "index.loadFailed": "无法加载项目。",
  "index.emptyTitle": "还没有项目",
  "index.emptyBody": "添加一个本地 Git 仓库，开始记录它的步骤。",
  "index.addProject": "添加项目",
  "index.filterArchived": "已归档",
  "index.sort": "排序",
  "index.sortName": "名称",
  "index.sortRemaining": "剩余步骤",
  "index.sortRecentCommit": "最近提交",
  "index.noMatch": "没有符合当前筛选条件的项目。",
  "overview.loading": "正在加载项目…",
  "overview.loadFailed": "无法加载此项目。",
  "rail.label": "项目导航", "rail.search": "搜索项目", "rail.allProjects": "全部项目", "rail.stepsLeft": "还剩 {count} 步",
  "rail.noMatch": "没有匹配的项目", "rail.collapse": "收起项目栏", "rail.expand": "展开项目栏",
  "rail.resize": "调整项目栏宽度", "settingsPage.kicker": "全局配置",
  "settingsPage.title": "设置",
  "settingsPage.description": "语言、提醒和 Agent 服务商的设置都在这里。",
  "notFound.eyebrow": "未知路径",
  "notFound.title": "找不到页面",
  "notFound.body": "页面位置可能已更改，但你的项目和本地状态没有受到影响。",
  "notFound.back": "返回项目列表",
  "row.notObserved": "尚未观测",
  "row.steps": "还剩 {open} / 共 {total} 步", "row.noSteps": "还没有步骤",
  "row.lastActivity": "最近提交 {time}",
  "row.changed": "{count} 项变更",
  "activity.summary": "最近 16 周共 {total} 次提交",
  "head.detached": "游离 HEAD",
  "head.unborn": "尚无提交",
  "head.branchUnborn": "{branch}（尚无提交）",
  "framing.setupTitle": "开始这个项目",
  "framing.title": "项目说明",
  "framing.setupIntro": "写下第一步，就可以开始了。",
  "framing.firstStep": "第一步",
  "framing.note": "一句话说明",
  "framing.optional": "可选",
  "framing.save": "保存",
  "framing.setupSuccess": "设置已完成。",
  "framing.saveSuccess": "已保存。",
  "framing.conflict": "你操作期间此项目已发生变化。已加载最新状态；请审视后重新提交，你输入的文本仍被保留。",
  "lifecycle.kicker": "项目状态",
  "lifecycle.title": "生命周期",
  "lifecycle.setStatus": "设置状态",
  "lifecycle.reason": "原因",
  "lifecycle.statusReason": "状态原因",
  "lifecycle.reviewDate": "审视日期",
  "lifecycle.reviewDateOptional": "审视日期（可选）",
  "lifecycle.confirmArchive": "确认归档",
  "lifecycle.archiveNotice": "我了解归档后，此项目将不再出现在默认列表中。",
  "lifecycle.update": "更新状态", "lifecycle.updateDisabled": "这个状态还需要填写原因、审视日期或归档确认",
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
  "task.advance": "让 AI 拆成子步骤", "task.advancing": "正在生成…", "task.advanceFailed": "拆解失败，请检查设置里的 Agent 配置。", "task.advanceReady": "为「{text}」生成了这些子步骤，勾选要采纳的：", "task.adoptSelected": "采纳所选", "task.proposal": "AI 拆解候选",
  "outline.title": "步骤", "outline.remaining": "还剩 {count} / 共 {total}", "outline.empty": "还没有步骤。写下第一步。", "outline.addStep": "新增一步", "outline.addDisabled": "先写下这一步的内容", "outline.addPlaceholder": "写下一步…", "outline.editStep": "编辑步骤内容", "outline.toggleDone": "完成：{text}", "outline.showDetails": "详情", "outline.hideDetails": "收起", "outline.removeStep": "删除这一步（含子步骤）", "outline.commitCount": "{count} 次提交", "outline.keys": "回车新增一行 · Tab 缩进为子步骤 · Shift+Tab 退回上一层 · ⌥↑ ⌥↓ 调整顺序",
  "project.lastCommit": "最近提交 {time}", "project.commits": "提交记录与分支图", "project.settings": "项目设置",
  "heatmap.loading": "正在读取提交记录…", "heatmap.summary": "过去一年 {total} 次提交，分布在 {days} 天", "heatmap.cell": "{date}：{count} 次提交", "heatmap.less": "少", "heatmap.more": "多",
  "task.unclear": "未成形（?）", "task.loading": "正在加载任务…", "task.due": "预期完成日期", "task.note": "问题备注", "task.conflict": "任务文件已发生变化，已重新加载；请审视后再次保存。", "task.tags": "标签", "date.today": "今天", "date.tomorrow": "明天", "date.friday": "本周五", "date.nextMonday": "下周一", "date.clear": "清除日期", "tags.placeholder": "输入或选择标签", "tags.remove": "移除标签 {tag}", "tags.suggestions": "本项目已用标签", "tags.full": "最多 {max} 个标签",
  "focus.title": "今日聚焦", "focus.summary": "{projects} 个项目共 {items} 条任务逾期或今日到期", "board.overdue": "逾期 {days} 天", "board.dueSoon": "{days} 天后到期", "board.dueToday": "今天到期",
  "timeline.kicker": "Git 实际", "timeline.title": "提交时间线", "timeline.loading": "正在加载提交…", "timeline.empty": "暂无可显示的提交。", "timeline.attributed": "已归属任务：{ids}",
  "timeline.assign": "归属提交 {sha}", "timeline.assignNone": "选择归属任务",
  "attention.count": "待关注项目：{count}",
  "settings.kicker": "提醒设置", "settings.title": "提醒", "settings.enabled": "启用提醒", "settings.cadence": "提醒频率", "settings.daily": "每天", "settings.off": "关闭", "settings.threshold": "静默阈值（天）", "settings.save": "保存设置", "settings.test": "发送测试提醒", "settings.saved": "提醒设置已保存。", "settings.tested": "测试提醒已发送。",
  "agent.kicker": "Agent 设置", "agent.title": "拆解模型", "agent.ready": "可用", "agent.notReady": "未就绪", "agent.privacy": "远程 Advance 只发送任务文本和问题备注；API key 保存在系统钥匙串，不写入 OmniProj 数据目录。", "agent.provider": "Provider（服务商）", "agent.model": "模型", "agent.apiKey": "API key", "agent.keyStored": "已保存于系统钥匙串；留空保持不变", "agent.keyRequired": "需要 API key", "agent.local": "本地", "agent.consent": "我同意把任务文本和问题备注发送给所选远程 provider。", "agent.save": "保存 Agent 设置", "agent.test": "测试连接", "agent.testDisabled": "先保存 Agent 设置并确认可用，才能测试连接", "agent.saveDisabled": "请先填写模型名称", "agent.testing": "正在测试…", "agent.saved": "Agent 设置已保存。", "agent.tested": "Agent 连接正常。", "agent.saveFailed": "无法保存 Agent 设置。", "agent.testFailed": "Agent 连接测试失败。",
  "graph.kicker": "Git 实际", "graph.title": "提交拓扑摘要", "graph.loading": "正在加载提交拓扑…", "graph.empty": "暂无可显示的 Git 提交拓扑。",
} as const;

type MessageKey = keyof typeof zh;
type Params = Record<string, string | number>;

const en: Record<MessageKey, string> = {
  "language.label": "Interface language", "language.zh": "中文", "language.en": "English",
  "common.cancel": "Cancel", "common.tryAgain": "Try again",
  "shell.backProjects": "Back to projects", "shell.filterProjects": "Filter projects", "shell.searchProjects": "Search projects", "shell.projects": "Projects", "shell.settings": "Settings", "shell.newProject": "New project", "shell.refresh": "Refresh", "shell.refreshing": "Refreshing projects", "shell.upToDate": "Projects are up to date.", "shell.refreshStarted": "Refreshing projects…", "shell.refreshed": "Projects refreshed.", "shell.refreshFailed": "{count} project(s) could not be refreshed. Last known facts were preserved.",
  "index.workspace": "Projects", "index.summary": "One list of steps per project, next to that project's git activity.", "index.projectCount": "{count} project(s)", "index.projectCountUnit": "projects", "index.loading": "Loading projects…", "index.loadFailed": "Couldn't load projects.", "index.emptyTitle": "No projects yet", "index.emptyBody": "Add a local Git repository and start writing down its steps.", "index.addProject": "Add project", "index.filterArchived": "Archived", "index.sort": "Sort", "index.sortName": "Name", "index.sortRecentCommit": "Recent commit", "index.sortRemaining": "Steps left", "index.noMatch": "No projects match this filter.", "overview.loading": "Loading project…", "overview.loadFailed": "Couldn't load this project.", "rail.label": "Project navigation", "rail.search": "Search projects", "rail.allProjects": "All projects", "rail.stepsLeft": "{count} steps left", "rail.noMatch": "No matching project", "rail.collapse": "Collapse project rail", "rail.expand": "Expand project rail", "rail.resize": "Resize project rail", "settingsPage.kicker": "Global configuration", "settingsPage.title": "Settings", "settingsPage.description": "Language, reminders, and the Agent provider live here.",
  "notFound.eyebrow": "Unknown route", "notFound.title": "Page not found", "notFound.body": "The page may have moved, but your projects and local state are unchanged.", "notFound.back": "Back to Projects",
  "row.steps": "{open} of {total} steps left", "row.noSteps": "No steps yet",
  "row.notObserved": "Not yet observed", "row.changed": "{count} changed", "row.lastActivity": "Last commit {time}", "activity.summary": "{total} commits in the last 16 weeks", "head.detached": "Detached HEAD", "head.unborn": "Unborn (no commits yet)", "head.branchUnborn": "{branch} (unborn, no commits yet)",
  "mutation.auditFailed": "State saved; audit commit failed.", "mutation.conflict": "This project changed since you started. Review the latest and resubmit.",
  "framing.setupTitle": "Start this project", "framing.title": "Project note", "framing.setupIntro": "Write the first step and you are going.", "framing.firstStep": "First step", "framing.note": "One-line note", "framing.optional": "Optional", "framing.save": "Save", "framing.setupSuccess": "Setup complete.", "framing.saveSuccess": "Saved.", "framing.conflict": "This project changed since you started. The latest state is loaded; review and resubmit — your text is kept.",
  "lifecycle.kicker": "Project state", "lifecycle.title": "Lifecycle", "lifecycle.setStatus": "Set status", "lifecycle.reason": "Reason", "lifecycle.statusReason": "Status reason", "lifecycle.reviewDate": "Review date", "lifecycle.reviewDateOptional": "Review date (optional)", "lifecycle.confirmArchive": "Confirm archive", "lifecycle.archiveNotice": "I understand archiving hides this project from the default list.", "lifecycle.update": "Update status", "lifecycle.updateDisabled": "This status still needs a reason, a review date, or the archive confirmation", "lifecycle.success": "Project status updated.",
  "recovery.kicker": "Recovery required", "recovery.title": "Source unavailable", "recovery.description": "This project's source is {status}. Point it at the repository's new location to restore observations — the project keeps its identity and history.", "recovery.choose": "Choose new location…", "recovery.duplicate": "That folder is already registered as “{name}”.", "recovery.invalid": "That folder can't be used ({state}).", "recovery.confirm": "Confirm relink", "recovery.confirmNotice": "I confirm this is the same project's repository.", "recovery.relink": "Relink source", "recovery.success": "Source relinked.",
  "add.title": "Add Project", "add.kicker": "Local Git repository", "add.description": "Register a repository without changing anything inside it.", "add.close": "Close Add Project", "add.choose": "Choose directory", "add.chooseAgain": "Choose directory again", "add.chooseProject": "Choose project directory", "add.chooseDifferent": "Choose a different directory", "add.readonly": "OmniProj reads Git facts and keeps the repository itself read-only.", "add.ready": "Repository ready", "add.noCommits": "No commits yet", "add.projectName": "Project name", "add.duplicate": "Already registered as “{name}”.", "add.openExisting": "Open existing project", "add.register": "Register", "add.registered": "Project registered.", "add.missing": "That folder no longer exists.", "add.unreadable": "That folder can't be read. Check permissions and try again.", "add.notGit": "That folder isn't a Git repository.", "add.bare": "Bare Git repositories aren't supported.", "add.observationFailed": "Couldn't read the repository: {message}", "add.validateFailed": "Couldn't validate that folder.", "add.registerFailed": "Couldn't register that project.",
  "task.advance": "Break into sub-steps with AI", "task.advancing": "Generating…", "task.advanceFailed": "Breakdown failed. Check the Agent settings.", "task.advanceReady": "Sub-steps generated for “{text}”. Tick the ones to keep:", "task.adoptSelected": "Adopt selected", "task.proposal": "AI breakdown candidates",
  "outline.title": "Steps", "outline.remaining": "{count} left of {total}", "outline.empty": "No steps yet. Write the first one.", "outline.addStep": "Add step", "outline.addDisabled": "Write the step first", "outline.addPlaceholder": "Write the next step…", "outline.editStep": "Edit step text", "outline.toggleDone": "Done: {text}", "outline.showDetails": "Details", "outline.hideDetails": "Hide", "outline.removeStep": "Delete this step (and its sub-steps)", "outline.commitCount": "{count} commits", "outline.keys": "Enter for a new line · Tab to nest · Shift+Tab to pop out · ⌥↑ ⌥↓ to reorder",
  "project.lastCommit": "last commit {time}", "project.commits": "Commits and branch graph", "project.settings": "Project settings",
  "heatmap.loading": "Reading commits…", "heatmap.summary": "{total} commits over the past year, on {days} days", "heatmap.cell": "{date}: {count} commits", "heatmap.less": "Less", "heatmap.more": "More",
  "task.unclear": "Not yet clear (?)", "task.loading": "Loading tasks…", "task.due": "Expected completion date", "task.note": "Problem note", "task.conflict": "The task file changed and was reloaded. Review it before saving again.", "task.tags": "Tags", "date.today": "Today", "date.tomorrow": "Tomorrow", "date.friday": "This Friday", "date.nextMonday": "Next Monday", "date.clear": "Clear date", "tags.placeholder": "Type or pick a tag", "tags.remove": "Remove tag {tag}", "tags.suggestions": "Tags used in this project", "tags.full": "Up to {max} tags",
  "focus.title": "Today's focus", "focus.summary": "{items} task(s) overdue or due today across {projects} project(s)", "board.overdue": "Overdue {days}d", "board.dueSoon": "Due in {days}d", "board.dueToday": "Due today",
  "timeline.kicker": "Git actual", "timeline.title": "Commit timeline", "timeline.loading": "Loading commits…", "timeline.empty": "No commits to show.", "timeline.attributed": "Attributed tasks: {ids}",
  "timeline.assign": "Attribute commit {sha}", "timeline.assignNone": "Choose a task",
  "attention.count": "Projects needing attention: {count}",
  "settings.kicker": "Reminder settings", "settings.title": "Reminders", "settings.enabled": "Enable reminders", "settings.cadence": "Reminder cadence", "settings.daily": "Daily", "settings.off": "Off", "settings.threshold": "Silence threshold (days)", "settings.save": "Save settings", "settings.test": "Send test reminder", "settings.saved": "Reminder settings saved.", "settings.tested": "Test reminder sent.",
  "agent.kicker": "Agent settings", "agent.title": "Breakdown model", "agent.ready": "Ready", "agent.notReady": "Not ready", "agent.privacy": "Remote Advance sends only the task text and problem note. API keys stay in the system credential store and are never written to OmniProj data.", "agent.provider": "Provider", "agent.model": "Model", "agent.apiKey": "API key", "agent.keyStored": "Stored in system keychain; leave blank to keep it", "agent.keyRequired": "API key required", "agent.local": "local", "agent.consent": "I agree to send task text and problem notes to the selected remote provider.", "agent.save": "Save Agent settings", "agent.test": "Test connection", "agent.testDisabled": "Save the Agent settings and make them ready before testing", "agent.saveDisabled": "Enter a model name first", "agent.testing": "Testing…", "agent.saved": "Agent settings saved.", "agent.tested": "Agent connection is ready.", "agent.saveFailed": "Couldn't save Agent settings.", "agent.testFailed": "Agent connection test failed.",
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
