//! Deep reasoning pipeline — goal #2 全面智能 (spec §5.2). Opt-in behind the
//! 推理深度 knob; the default path stays the single-pass distill.
//!
//! Three enhancements, same trust pipeline (everything still flows through the
//! deterministic verify gate afterwards):
//! 1. **map-reduce older sessions** — sessions beyond the digest's recency window
//!    are individually compressed (map) and fed to synthesis (reduce), replacing
//!    "truncate and discard" (the §4.2 recency-cap blind spot).
//! 2. **structured extraction before prose** — first pull a timeline / evidenced
//!    blockers / decision candidates as a structured intermediate, then write
//!    prose anchored on it.
//! 3. **completeness critic** — one inference-time self-check over the draft:
//!    "what's missing? what has no FactSheet basis?" → revised final.
//!
//! Cost: 1 (extract) + 1 (distill) + 1 (critic) + ≤MAP_CAP (compress) calls.

use crate::provider::LlmProvider;
use crate::DistillOutput;
use omniproj_core::FactSheet;

/// Upper bound on map-pass compression calls per distill (cost guard).
pub const MAP_CAP: usize = 6;

const COMPRESS_SYSTEM_PROMPT: &str = r#"你是 session 压缩器。输入是一段较旧的工作会话(user/assistant 对话)。压缩成 ≤150 字的中文摘要,只保留对「项目状态」有意义的内容:做了什么、决定了什么、卡在哪、留下什么未完成。忽略寒暄、跑题、调试噪音。没有实质内容就输出「(无实质进展)」。直接输出摘要,无标题无解释。"#;

/// Map pass: compress one older session's rendered text.
pub async fn compress_session(
    session_text: &str,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    provider
        .complete(COMPRESS_SYSTEM_PROMPT, session_text)
        .await
}

const EXTRACT_SYSTEM_PROMPT: &str = r#"你是结构化抽取器(蒸馏前的中间趟)。输入:VERIFIED FACTS(代码核验的 git 事实)+ substrate digest。任务:先于散文,把状态「抽取成结构」,为后续蒸馏提供锚点。

输出以下四节 markdown(空节写「(无)」),每条尽量短、带证据指向(commit hash 须出自 VERIFIED FACTS 白名单,引不到就不引):

## TIMELINE
- <日期> <hash?> <一句话:发生了什么>   (按时间升序,只列状态有意义的节点)

## BLOCKERS
- <卡点> — 证据: <digest 里的真实依据:报错/退出码/对话原文要点>

## DECISION-CANDIDATES
- <已做出的判断/结论> — 依据: <...>

## LOOSE-ENDS
- <未闭环的线头:提过但没下文、计划了没做的>

只输出这四节,不写散文,不建议,不编造。"#;

/// Extraction pass: digest → structured intermediate (markdown).
pub async fn extract_structure(
    digest: &str,
    facts: &FactSheet,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let msg = format!("{}\n{digest}", crate::prompt::render_facts(facts));
    provider.complete(EXTRACT_SYSTEM_PROMPT, &msg).await
}

const CRITIC_SYSTEM_PROMPT: &str = r#"你是完整性 critic(蒸馏后的自检趟)。输入:草稿(briefing/decisions/open 三段)、结构化抽取、VERIFIED FACTS、substrate digest。

自检两问并直接修订:
1. **漏了什么?** 对照结构化抽取与 digest:有没有 BLOCKERS / DECISION-CANDIDATES / LOOSE-ENDS 在草稿里失踪了?补进对应段落。
2. **哪条没依据?** 草稿里有没有 digest/FACTS 撑不住的断言、白名单外的 hash、来历不明的数字?改成「未知」、删除或标「AI 推断」。

约束:保持草稿原有结构与详略风格,只做增漏与去虚;不加建议、不排优先级;briefing 仍须 <60 秒读完。

输出修订后的完整三段,沿用相同分隔标记:
===BRIEFING===
...
===DECISIONS===
...
===OPEN===
..."#;

/// Critic pass: draft + extraction + digest → revised three sections.
pub async fn critic_pass(
    draft: &DistillOutput,
    extraction: &str,
    digest: &str,
    facts: &FactSheet,
    provider: &impl LlmProvider,
) -> anyhow::Result<DistillOutput> {
    let msg = format!(
        "## 草稿\n===BRIEFING===\n{}\n===DECISIONS===\n{}\n===OPEN===\n{}\n\n## 结构化抽取\n{}\n\n{}\n{digest}",
        draft.briefing,
        draft.decisions,
        draft.open,
        extraction,
        crate::prompt::render_facts(facts),
    );
    let raw = provider.complete(CRITIC_SYSTEM_PROMPT, &msg).await?;
    Ok(crate::parse_output(&raw))
}

/// Assemble the augmented digest for deep synthesis: compressed older sessions
/// (the reduce input) + the structured extraction, ahead of the normal digest.
/// Pure — unit-testable without a provider.
pub fn augment_digest(digest: &str, older_summaries: &[String], extraction: &str) -> String {
    let mut out = String::new();
    if !older_summaries.is_empty() {
        out.push_str("## OLDER SESSIONS(超出近期窗口的旧会话,已逐个压缩 — map-reduce)\n");
        for (i, s) in older_summaries.iter().enumerate() {
            out.push_str(&format!("- [旧#{i}] {}\n", s.trim()));
        }
        out.push('\n');
    }
    if !extraction.trim().is_empty() {
        out.push_str("## STRUCTURED EXTRACTION(先抽取的结构化中间态,散文请以此为锚)\n");
        out.push_str(extraction.trim());
        out.push_str("\n\n");
    }
    out.push_str(digest);
    out
}

/// The deep pipeline: map(compress older) → extract → distill(augmented) → critic.
/// `older_session_texts` are pre-rendered texts of sessions OUTSIDE the digest's
/// recency window, newest-first; at most [`MAP_CAP`] are compressed. `log` receives
/// per-pass progress lines.
pub async fn distill_deep(
    digest: &str,
    facts: &FactSheet,
    learned: &str,
    user_model: &str,
    older_session_texts: &[String],
    provider: &impl LlmProvider,
    log: impl Fn(&str),
) -> anyhow::Result<DistillOutput> {
    // Map: compress the older sessions the shallow digest would have discarded.
    let mut older_summaries = Vec::new();
    let take = older_session_texts.len().min(MAP_CAP);
    if older_session_texts.len() > take {
        log(&format!(
            "deep/map: compressing {take} of {} older sessions (cap {MAP_CAP})",
            older_session_texts.len()
        ));
    } else if take > 0 {
        log(&format!("deep/map: compressing {take} older session(s)"));
    }
    for text in older_session_texts.iter().take(take) {
        match compress_session(text, provider).await {
            Ok(s) if !s.trim().is_empty() => older_summaries.push(s),
            Ok(_) => {}
            // One bad compression shouldn't sink the whole distill.
            Err(e) => log(&format!("deep/map: a session failed to compress: {e:#}")),
        }
    }

    // Extract: structure before prose.
    log("deep/extract: structured extraction…");
    let extraction = extract_structure(digest, facts, provider).await?;

    // Reduce/synthesize: the normal grounded distill over the augmented digest.
    log("deep/distill: synthesizing draft…");
    let augmented = augment_digest(digest, &older_summaries, &extraction);
    let draft = crate::distill(&augmented, facts, learned, user_model, provider).await?;

    // Critic: completeness + groundedness self-check, revising in place.
    log("deep/critic: completeness check…");
    critic_pass(&draft, &extraction, &augmented, facts, provider).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn augment_orders_blocks_before_digest() {
        let out = augment_digest(
            "# SUBSTRATE DIGEST\nbody",
            &["older one".into(), "older two".into()],
            "## TIMELINE\n- t1",
        );
        let older = out.find("OLDER SESSIONS").unwrap();
        let extract = out.find("STRUCTURED EXTRACTION").unwrap();
        let digest = out.find("# SUBSTRATE DIGEST").unwrap();
        assert!(older < extract && extract < digest);
        assert!(out.contains("[旧#0] older one") && out.contains("[旧#1] older two"));
    }

    #[test]
    fn augment_with_nothing_extra_is_identity() {
        assert_eq!(augment_digest("d", &[], ""), "d");
    }
}
