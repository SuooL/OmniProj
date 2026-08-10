//! Clarify — the per-item adversarial discussion (cockpit; charter §6 例外).
//!
//! When a next-action item is 未成形 (the user flagged it `?`), `omniproj clarify` runs a
//! bounded, user-initiated discussion **scoped to that one item**. Output is 「标记 +
//! 理由」 in the same discipline as second-opinion (charter §5 原则3): it surfaces
//! unstated premises, internal contradictions, and missing falsifiable criteria — it
//! does NOT recommend an answer, pick a direction, or write the item for the user.
//!
//! This is the one place charter §6's "no chat agent" rule is relaxed, and only under
//! its guardrails: the scope is a single item, the round count is surfaced to the user
//! (§10 monitoring), the discussion product is AI derivative (`auto/clarify/<id>.md`),
//! and the conclusion is NEVER auto-written to `notes/` — the user transcribes it, so
//! authorship of "想清楚了" stays theirs.

use crate::provider::LlmProvider;

pub const CLARIFY_SYSTEM_PROMPT: &str = r#"你是一个「把想法逼清楚」的对谈者,不是给答案的助理。用户抛来一条他自己还没想清楚的待办事项,你的唯一职责是帮他把它想清楚——通过提问和标记,不是通过替他拍板。

硬规则:
1. 输出是「标记 + 理由」,不是建议或结论。允许的形态:指出未定义的前提、互相矛盾的假设、缺失的可证伪判据、边界不清的范围、这条事项其实藏着的几个不同子问题。**绝不**说「你应该做 X」「建议先做 Y」「正确做法是 Z」——收敛由用户自己完成。
2. 每条 2-4 行:先一句**标记**(你观察到的模糊/张力/缺口),再一句**理由**(为什么它挡住了「想清楚」)。
3. 可以提问,但提的是让用户自己回答的问题,不是你自问自答后给结论。
4. 3-5 条为宜,宁缺毋滥。找不到真实的模糊点就说「这条其实已经相当清楚了,主要待定的只有:…」而不是硬凑。
5. 简洁,中文,markdown。不要寒暄、不要复述用户原话、不要在结尾总结或催促下一步。"#;

/// One clarify round. `prior` is the accumulated discussion so far (empty on round 1);
/// `user_note` is an optional steer the user attached to THIS round (e.g. answering a
/// question the model raised last time). Returns the model's 标记+理由 for this round.
pub async fn clarify_round(
    item_text: &str,
    prior: &str,
    user_note: Option<&str>,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let prior_block = if prior.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n## 到目前为止的讨论(供你接续,不要重复)\n{}\n",
            prior.trim()
        )
    };
    let note_block = match user_note {
        Some(n) if !n.trim().is_empty() => {
            format!("\n## 用户这一轮补充的想法\n{}\n", n.trim())
        }
        _ => String::new(),
    };
    let msg = format!(
        "## 待想清楚的事项\n{}\n{prior_block}{note_block}",
        item_text.trim()
    );
    provider.complete(CLARIFY_SYSTEM_PROMPT, &msg).await
}

/// Render one round for appending to `auto/clarify/<id>.md`. `at` is an RFC3339
/// timestamp supplied by the caller (distill stays clock-free at the seam). The
/// machine-readable `<!--round @ ...-->` marker lets the round counter (§10 monitoring)
/// tally rounds by week without parsing prose.
pub fn render_round(at: &str, user_note: Option<&str>, model_text: &str) -> String {
    let mut out = format!("\n## round <!--round @ {at}-->\n");
    if let Some(n) = user_note {
        if !n.trim().is_empty() {
            out.push_str(&format!("**你补充：** {}\n\n", n.trim()));
        }
    }
    out.push_str(model_text.trim());
    out.push('\n');
    out
}

/// Count clarify rounds recorded across a discussion file within the last `window_secs`
/// of `now_epoch`, by scanning `<!--round @ <rfc3339>-->` markers. Pure + testable;
/// powers the "本周 N 轮" self-monitoring counter (charter §10). `parse_epoch` converts
/// an RFC3339 stamp to epoch secs (injected so this stays clock/dep-free).
pub fn count_rounds_within(
    discussion: &str,
    now_epoch: i64,
    window_secs: i64,
    parse_epoch: impl Fn(&str) -> Option<i64>,
) -> usize {
    discussion
        .lines()
        .filter_map(round_marker_ts)
        .filter_map(parse_epoch)
        .filter(|t| now_epoch - t <= window_secs && now_epoch - t >= 0)
        .count()
}

/// Extract the RFC3339 timestamp from a `<!--round @ <ts>-->` marker line, if present.
fn round_marker_ts(line: &str) -> Option<&str> {
    let start = line.find("<!--round @ ")? + "<!--round @ ".len();
    let rest = &line[start..];
    let end = rest.find("-->")?;
    Some(rest[..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_round_carries_machine_readable_marker() {
        let r = render_round("2026-08-09T12:00:00Z", None, "  - 标记: 前提未定义\n  ");
        assert!(r.contains("<!--round @ 2026-08-09T12:00:00Z-->"));
        assert!(r.contains("- 标记: 前提未定义"));
        assert_eq!(
            round_marker_ts("## round <!--round @ 2026-08-09T12:00:00Z-->"),
            Some("2026-08-09T12:00:00Z")
        );
    }

    #[test]
    fn render_round_includes_user_note_when_present() {
        let r = render_round("2026-08-09T12:00:00Z", Some("我觉得应该先做 A"), "回应");
        assert!(r.contains("**你补充：** 我觉得应该先做 A"));
        let r2 = render_round("2026-08-09T12:00:00Z", Some("   "), "回应");
        assert!(!r2.contains("你补充")); // blank note is omitted
    }

    #[test]
    fn counts_rounds_only_within_window() {
        // Two markers: one 1 day ago, one 30 days ago. 7-day window → only the recent.
        let now = 1_000_000_000i64;
        let day = 86_400i64;
        let disc = "## round <!--round @ recent-->\n...\n## round <!--round @ old-->\n...\n";
        let parse = |s: &str| match s {
            "recent" => Some(now - day),
            "old" => Some(now - 30 * day),
            _ => None,
        };
        assert_eq!(count_rounds_within(disc, now, 7 * day, parse), 1);
        assert_eq!(count_rounds_within(disc, now, 60 * day, parse), 2);
    }

    #[test]
    fn count_ignores_unparseable_and_future_stamps() {
        let now = 1_000i64;
        let disc = "## round <!--round @ future-->\n## round <!--round @ junk-->\n";
        let parse = |s: &str| match s {
            "future" => Some(now + 500), // clock skew / future → excluded (now-t < 0)
            _ => None,                   // "junk" unparseable → excluded
        };
        assert_eq!(count_rounds_within(disc, now, 10_000, parse), 0);
    }
}
