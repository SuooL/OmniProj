//! Self-iteration: the learned loop (spec §5.3, Reflexion-style). A user correction
//! (free-text and/or a git diff of their in-place edit to the briefing) is distilled
//! into a bounded, deduplicated list of per-project heuristics that steer FUTURE
//! distillation. The user's correction is the gradient we'd otherwise throw away.

use crate::provider::LlmProvider;

const LEARN_SYSTEM: &str = r#"你是某个项目的「修正学习器」。输入是该项目已有的 learned 启发式(可能为空)和用户对最近 briefing 的一条修正(可能是自由文本、可能是一段 git diff、或两者)。你的任务:产出**更新后的 learned.md 正文**——一份精简、去重、有上限的 per-project 启发式清单,用来指导未来如何蒸馏这个项目。

铁律:
1. **只提炼「呈现/抽取偏好」**:如何呈现、突出什么、信哪个来源、措辞与详略、回来第一件事怎么写等。**绝不**包含对用户工作的价值判断或行动建议(不写「你该做 X」「这个方向不好」「优先做 Y」)。这是硬红线。
2. 每条一行,祈使句,具体可执行(坏:「写好点」;好:「briefing 顶部先写当前 blocker,再写进展」)。
3. 最多约 10 条;超了就把最相近的合并。新修正与旧条目冲突时,新的覆盖旧的。
4. 若这条修正不含任何可推广的呈现偏好(纯属一次性事实纠正),就原样返回已有清单、不强行加条目。
5. 只输出 learned.md 正文(一个 markdown 无序列表),不要任何解释、前言或代码围栏。"#;

/// Produce the updated `learned.md` body from existing heuristics + a correction
/// signal. `existing` may be empty (first correction).
pub async fn learn_from_correction(
    existing: &str,
    signal: &str,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let existing = if existing.trim().is_empty() {
        "(暂无已有启发式)"
    } else {
        existing.trim()
    };
    let user = format!(
        "## 已有 learned 启发式\n{existing}\n\n## 用户的新修正\n{}\n\n请据此输出更新后的 learned.md 正文。",
        signal.trim()
    );
    let out = provider.complete(LEARN_SYSTEM, &user).await?;
    Ok(out.trim().to_string())
}
