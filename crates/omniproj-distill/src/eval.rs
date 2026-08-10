//! Gold-eval harness (review P2#8, spec §10). Score a fresh distillation against a
//! human-written gold re-entry doc (the user's HANDOFF.md) with an LLM judge.
//!
//! This makes prompt iteration measurable: spike §10 established the golds and the
//! "~80% of hand-written content at zero cost" baseline; this harness re-checks
//! that claim any time the distill prompts / pipeline change. The judge only
//! scores — it never edits state. Reports persist to `cache/eval-report.json` so
//! score trends survive across runs.

use crate::provider::LlmProvider;
use serde_json::Value;

/// One graded dimension, 1–10.
#[derive(Debug, Clone)]
pub struct EvalScores {
    /// 事实一致: candidate contradicts/fabricates nothing vs the gold + its own facts.
    pub factual: u8,
    /// 覆盖度: how much of the gold's load-bearing content the candidate carries.
    pub coverage: u8,
    /// 简洁可读: re-entry usefulness per minute of reading.
    pub concision: u8,
    pub rationale: String,
}

const JUDGE_SYSTEM: &str = r#"你是蒸馏质量评审。输入两份同一项目的 re-entry 文档:
- GOLD:用户手写的 HANDOFF(真相参照,但可能略过期)
- CANDIDATE:OmniProj 自动蒸馏的 briefing/open/decisions

逐维打分(1-10,整数),只评 CANDIDATE:
1. factual 事实一致 —— CANDIDATE 是否与 GOLD 矛盾、或包含 GOLD/常理无法支持的具体断言(hash/数字/文件名尤其严格)。矛盾或编造越多分越低。注意:GOLD 可能过期,CANDIDATE 基于更新的 git 状态与 GOLD 不同不算错,除非内部自相矛盾。
2. coverage 覆盖度 —— GOLD 中「承重」内容(当前状态/卡点/决策/下一步)有多少在 CANDIDATE 里有对应。
3. concision 简洁可读 —— 单位阅读时间的 re-entry 信息量;冗长、重复、空话扣分。

只输出一个 JSON 对象,无其他文字:
{"factual": <1-10>, "coverage": <1-10>, "concision": <1-10>, "rationale": "<≤120字中文理由>"}"#;

/// Run the judge over (gold, candidate). Returns parsed scores.
pub async fn judge(
    gold: &str,
    candidate: &str,
    provider: &impl LlmProvider,
) -> anyhow::Result<EvalScores> {
    let user = format!(
        "## GOLD(手写 HANDOFF)\n{}\n\n## CANDIDATE(OmniProj 蒸馏)\n{}",
        gold.trim(),
        candidate.trim()
    );
    let raw = provider.complete(JUDGE_SYSTEM, &user).await?;
    parse_judge(&raw)
}

/// Parse the judge's JSON, tolerating prose/code fences around it (models drift).
/// Pure — unit-testable without a provider.
///
/// Fast path: strict `serde_json` parse of the `{...}` slice (unchanged behavior on
/// clean JSON). Fallback: real DeepSeek runs frequently emit a `rationale` string
/// containing **unescaped double-quotes** (Chinese prose like `含"..."有化虚为实之嫌`),
/// which makes the whole slice invalid JSON. The three integer scores are what the
/// eval actually needs (they feed the baseline + regression gate); the rationale is
/// informational. So on strict-parse failure we tolerantly scan the three integer
/// dimensions straight out of the raw text and best-effort recover the rationale,
/// rather than crashing on an otherwise perfectly readable judgement (dogfood finding).
pub fn parse_judge(raw: &str) -> anyhow::Result<EvalScores> {
    let start = raw
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("judge output has no JSON: {raw}"))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("judge output has no JSON: {raw}"))?;
    let slice = &raw[start..=end];

    // Fast path: clean JSON.
    if let Ok(v) = serde_json::from_str::<Value>(slice) {
        if let (Some(f), Some(c), Some(n)) = (
            v.get("factual").and_then(Value::as_u64),
            v.get("coverage").and_then(Value::as_u64),
            v.get("concision").and_then(Value::as_u64),
        ) {
            return Ok(EvalScores {
                factual: f.clamp(1, 10) as u8,
                coverage: c.clamp(1, 10) as u8,
                concision: n.clamp(1, 10) as u8,
                rationale: v
                    .get("rationale")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }

    // Fallback: tolerant scan for the three integer dimensions. Does NOT depend on
    // the rationale being valid JSON.
    let (Some(factual), Some(coverage), Some(concision)) = (
        scan_dim(slice, "factual"),
        scan_dim(slice, "coverage"),
        scan_dim(slice, "concision"),
    ) else {
        anyhow::bail!("judge JSON invalid (no readable scores): {raw}");
    };

    Ok(EvalScores {
        factual,
        coverage,
        concision,
        rationale: recover_rationale(slice),
    })
}

/// Find `"<key>"` then the first integer 1..=10 after the following `:`. Tolerant of
/// surrounding malformed JSON. Returns the clamped score, or `None` if absent.
fn scan_dim(slice: &str, key: &str) -> Option<u8> {
    let needle = format!("\"{key}\"");
    let after_key = &slice[slice.find(&needle)? + needle.len()..];
    let after_colon = &after_key[after_key.find(':')? + 1..];
    let digits: String = after_colon
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let n: u64 = digits.parse().ok()?;
    Some(n.clamp(1, 10) as u8)
}

/// Best-effort recover the rationale text after the last score field, up to the final
/// `}`. Strips the `"rationale"...:` wrapper and surrounding quotes if present. Returns
/// an empty string if it can't be cleanly located — a messy-but-present (or empty)
/// rationale is fine; the scores are what matter.
fn recover_rationale(slice: &str) -> String {
    let Some(idx) = slice.find("\"rationale\"") else {
        return String::new();
    };
    let after = &slice["\"rationale\"".len() + idx..];
    let Some(colon) = after.find(':') else {
        return String::new();
    };
    let body = after[colon + 1..].trim();
    // Drop trailing `}` and surrounding quotes, keeping inner (possibly unescaped) text.
    let body = body.strip_suffix('}').unwrap_or(body).trim();
    let body = body.strip_prefix('"').unwrap_or(body);
    let body = body.strip_suffix('"').unwrap_or(body);
    body.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LlmProvider;
    use std::sync::Mutex;

    #[test]
    fn parses_clean_json() {
        let s = parse_judge(r#"{"factual": 9, "coverage": 7, "concision": 8, "rationale": "好"}"#)
            .unwrap();
        assert_eq!((s.factual, s.coverage, s.concision), (9, 7, 8));
        assert_eq!(s.rationale, "好");
    }

    #[test]
    fn tolerates_fences_prose_and_clamps_range() {
        let raw = "评审如下:\n```json\n{\"factual\": 99, \"coverage\": 0, \"concision\": 5, \"rationale\": \"x\"}\n```\n以上。";
        let s = parse_judge(raw).unwrap();
        assert_eq!(s.factual, 10, "clamped down");
        assert_eq!(s.coverage, 1, "clamped up");
    }

    #[test]
    fn missing_dim_or_no_json_errors() {
        assert!(parse_judge("no json here").is_err());
        assert!(parse_judge(r#"{"factual": 5}"#).is_err());
    }

    /// Dogfood regression: a real DeepSeek run emitted a `rationale` full of
    /// unescaped double-quotes (Chinese prose), making the `{...}` slice invalid
    /// JSON. Strict parse used to ERROR OUT even though the three integer scores are
    /// perfectly readable. Use the EXACT failing string as the fixture.
    #[test]
    fn tolerates_unescaped_quotes_in_rationale() {
        let raw = r#"{"factual": 6, "coverage": 7, "concision": 8, "rationale": "事实一致: GOLD明确13外中心B锁C-N pending但CANDIDATE写稿件含"13外中心B-N描述"有化虚为实之嫌,且称CORN加权$US增强等GOLD无提及(若为新进展可接受但需注明来源);覆盖度:抓住了项目阶段/工作区/主要卡点但略过GOLD中投递策略/事实边界/标签来源等承重细节;简洁可读:结构清晰"回来第一件事"实用,但"最近进展"罗列commit略显冗余。"}"#;
        let s = parse_judge(raw).expect("must not error on malformed-rationale JSON");
        assert_eq!((s.factual, s.coverage, s.concision), (6, 7, 8));
        // rationale may be partial/messy; we only require it not to crash.
    }

    #[test]
    fn escaped_quote_rationale_parses_via_fast_path() {
        let s = parse_judge(
            r#"{"factual": 5, "coverage": 6, "concision": 7, "rationale": "含 \"引号\" 的理由"}"#,
        )
        .unwrap();
        assert_eq!((s.factual, s.coverage, s.concision), (5, 6, 7));
        assert_eq!(s.rationale, r#"含 "引号" 的理由"#);
    }

    #[test]
    fn garbage_with_braces_but_no_scores_errors() {
        assert!(parse_judge(r#"{"note": "totally unrelated blob"}"#).is_err());
    }

    /// Canned-JSON test double so `judge()`'s non-LLM wiring (prompt assembly +
    /// score parsing) is testable without a real provider. Records the user prompt.
    struct JudgeMock {
        response: String,
        last_user: Mutex<Option<String>>,
    }

    impl LlmProvider for JudgeMock {
        async fn complete(&self, _system: &str, user: &str) -> anyhow::Result<String> {
            *self.last_user.lock().expect("mock lock") = Some(user.to_string());
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn judge_assembles_prompt_and_parses_scores() {
        let mock = JudgeMock {
            response: r#"{"factual": 8, "coverage": 6, "concision": 9, "rationale": "ok"}"#
                .to_string(),
            last_user: Mutex::new(None),
        };
        let scores = judge("GOLD-HANDOFF-MARKER", "CANDIDATE-BRIEFING-MARKER", &mock)
            .await
            .expect("mock never fails");
        assert_eq!(
            (scores.factual, scores.coverage, scores.concision),
            (8, 6, 9)
        );
        let user = mock.last_user.lock().unwrap().clone().expect("recorded");
        assert!(
            user.contains("GOLD-HANDOFF-MARKER"),
            "gold in the judge prompt"
        );
        assert!(
            user.contains("CANDIDATE-BRIEFING-MARKER"),
            "candidate in the judge prompt"
        );
    }
}
