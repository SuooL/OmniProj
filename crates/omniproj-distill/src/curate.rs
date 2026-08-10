//! Self-iteration: the Curator GC pass (spec §5.3, Hermes-curator-style). A batched,
//! offline consolidation decoupled from live distillation, covering the three
//! curation targets the spec names (review P2#7):
//!  - `decisions.md` — append-only judgment log → merge/supersede (LLM);
//!  - `open.md`      — open-threads list → drop resolved/stale, merge dupes (LLM);
//!  - `learned.md`   — correction heuristics → consolidate ONLY past a hard cap;
//!  - user model     — USER-owned (charter §5 原则4): never rewritten by AI; the
//!    curator only WARNS when a dimension exceeds its budget.
//!
//! Every pass only consolidates existing content — never invents — so it stays
//! trustworthy without a FactSheet re-check.

use crate::provider::LlmProvider;

/// learned.md hard cap (chars). Under it, the file is left untouched — small
/// heuristic lists need no GC and every LLM pass risks drift.
pub const LEARNED_CAP_CHARS: usize = 3_000;
pub use omniproj_core::USER_MODEL_DIM_CAP_CHARS;

const CURATE_SYSTEM: &str = r#"你是某个项目 decisions.md 的「整理器(Curator)」。decisions.md 是 append-only 的「已做判断/结论」清单,随时间累积,会出现重复、被后续推翻、或同一决策的多次表述。你的任务:产出**整合后的 decisions.md 正文**。

铁律:
1. **只整合已有内容,绝不新增决策、绝不臆测**。输入里没有的判断,输出里也不能有。
2. 合并语义等价的条目;**保留每条的日期/时间戳**(取最早出现的日期)。
3. 若两条决策矛盾,保留**最新**的结论,并简短注明它取代了早先的判断。
4. 保持时间顺序(或按主题分组但组内有序)。简洁,不加评论。
5. 只输出整合后的 decisions.md 正文(markdown),不要任何解释、前言或代码围栏。"#;

/// Consolidate an append-only `decisions.md` body. Returns the curated body.
pub async fn curate_decisions(
    decisions: &str,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let user = format!(
        "下面是当前(append-only)的 decisions.md。请整合去重后输出新的正文。\n\n{}",
        decisions.trim()
    );
    let out = provider.complete(CURATE_SYSTEM, &user).await?;
    Ok(out.trim().to_string())
}

const OPEN_SYSTEM: &str = r#"你是某个项目 open.md 的「整理器(Curator)」。open.md 是「未闭环卡点/待决问题」清单,随时间累积,会出现重复、文中已自述解决/失效的条目。你的任务:产出**整理后的 open.md 正文**。

铁律:
1. **只整理已有内容,绝不新增卡点、绝不臆测进展**。
2. 合并语义重复的条目,保留日期与「回来第一件事」提示。
3. 仅当条目**自身文字表明**已解决/已失效时才移除;不确定的一律保留(宁多勿删)。
4. 保持简洁有序。只输出整理后的 open.md 正文(markdown),无解释、无代码围栏。"#;

/// GC an `open.md` body: merge duplicates, drop only self-evidently resolved items.
pub async fn curate_open(open: &str, provider: &impl LlmProvider) -> anyhow::Result<String> {
    let user = format!(
        "下面是当前的 open.md。请按铁律整理后输出新的正文。\n\n{}",
        open.trim()
    );
    let out = provider.complete(OPEN_SYSTEM, &user).await?;
    Ok(out.trim().to_string())
}

const LEARNED_SYSTEM: &str = r#"你是某个项目 learned.md 的「整理器(Curator)」。learned.md 是从用户修正中蒸出的呈现/抽取偏好启发式清单,已超出预算上限。你的任务:**压缩整合**为更精炼的版本。

铁律:
1. **不丢失任何独立的偏好语义**——合并相似项、删除完全重复项、收紧措辞。
2. 后出现的修正优先于早先矛盾的修正。
3. 输出必须明显短于输入。只输出整合后的 learned.md 正文(markdown),无解释。"#;

/// Whether learned.md needs consolidation (pure, testable).
pub fn learned_over_cap(learned: &str) -> bool {
    learned.chars().count() > LEARNED_CAP_CHARS
}

/// Consolidate an over-cap `learned.md`. Caller checks [`learned_over_cap`] first.
pub async fn consolidate_learned(
    learned: &str,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let user = format!(
        "下面是超出 {LEARNED_CAP_CHARS} 字预算的 learned.md(当前 {} 字)。请压缩整合。\n\n{}",
        learned.chars().count(),
        learned.trim()
    );
    let out = provider.complete(LEARNED_SYSTEM, &user).await?;
    Ok(out.trim().to_string())
}

/// User-model dimensions over their per-dimension budget — returned as warnings,
/// never edited (the model file is user-owned, charter §5 原则4). Pure.
pub fn user_model_over_cap(model: &omniproj_core::UserModel) -> Vec<(String, usize)> {
    model
        .dimensions
        .iter()
        .filter(|d| d.enabled && d.body.chars().count() > USER_MODEL_DIM_CAP_CHARS)
        .map(|d| (d.name.clone(), d.body.chars().count()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learned_cap_is_char_based() {
        assert!(!learned_over_cap("short"));
        assert!(!learned_over_cap(&"中".repeat(LEARNED_CAP_CHARS)));
        assert!(learned_over_cap(&"中".repeat(LEARNED_CAP_CHARS + 1)));
    }

    #[test]
    fn user_model_cap_flags_only_enabled_oversized_dims() {
        let text = format!(
            "## domain_prior\n{}\n\n## risk_pref (disabled)\n{}\n\n## presentation_pref\nshort\n",
            "x".repeat(USER_MODEL_DIM_CAP_CHARS + 10),
            "y".repeat(USER_MODEL_DIM_CAP_CHARS + 10),
        );
        let model = omniproj_core::UserModel::parse(&text);
        let over = user_model_over_cap(&model);
        assert_eq!(over.len(), 1, "disabled + small dims must not be flagged");
        assert_eq!(over[0].0, "domain_prior");
    }
}
