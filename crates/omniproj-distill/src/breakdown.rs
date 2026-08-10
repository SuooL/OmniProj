//! Advance — task breakdown (推进层, FR-V1). Given a not-yet-executable next-action, the
//! agent proposes 3-6 concrete, executable sub-steps. Human-in-the-loop (charter §4b / §5
//! 原则3): this is a PROPOSAL (AI derivative in `auto/`), never auto-written to `notes/` —
//! the user picks which to adopt. 推荐权给 agent,拍板权与执行权留给人.

use crate::provider::LlmProvider;

pub const BREAKDOWN_SYSTEM_PROMPT: &str = r#"你是一个把「想不清、推不动」的待办拆成「今天就能上手」的具体步骤的拆解器。用户给你一条还没法直接动手的任务,你的职责是把它拆成 3-6 条**具体、可执行**的下一步。

硬规则:
1. 每条是一个**具体动作**,读完就知道手往哪儿放(能对应到一次提交/一个函数/一处改动/一次实验/一份查证),不是「研究一下 X」「考虑 Y」这种还要再拆的空话。
2. 每条一行,动词开头,不编号(编号我来),不加解释性长句。
3. 3-6 条;顺序尽量按能落地的先后。
4. 只拆解,不评判、不推荐优先级、不替用户决定做不做——用户会自己挑哪几条采纳。
5. 只输出这个列表(每行一条,可用 `- ` 前缀),不要前言、不要结尾总结。
中文,除非任务本身是英文语境。"#;

/// Break a next-action into concrete candidate sub-steps (FR-V1). `context` is optional
/// extra grounding (e.g. the item's problem note). Returns the parsed candidate list.
pub async fn breakdown(
    task: &str,
    context: Option<&str>,
    provider: &impl LlmProvider,
) -> anyhow::Result<Vec<String>> {
    let ctx = match context {
        Some(c) if !c.trim().is_empty() => format!("\n## 相关备注\n{}\n", c.trim()),
        _ => String::new(),
    };
    let msg = format!("## 待拆解的任务\n{}\n{ctx}", task.trim());
    let raw = provider.complete(BREAKDOWN_SYSTEM_PROMPT, &msg).await?;
    Ok(parse_steps(&raw))
}

/// Parse a model list-reply into clean one-line steps: strip bullets / numbering /
/// surrounding whitespace, and drop empties, headers (`… :`), and too-short noise.
pub fn parse_steps(raw: &str) -> Vec<String> {
    raw.lines()
        .map(strip_list_marker)
        .map(str::trim)
        .filter(|s| s.chars().count() >= 3 && !s.ends_with(':') && !s.ends_with('：'))
        .map(str::to_string)
        .collect()
}

/// Strip a leading list marker (`- `, `* `, `• `, `1. `, `1) `, `1、`) if present.
fn strip_list_marker(line: &str) -> &str {
    let t = line.trim();
    for m in ["- ", "* ", "• ", "· "] {
        if let Some(r) = t.strip_prefix(m) {
            return r;
        }
    }
    let digits: usize = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &t[digits..];
        for m in [". ", ") ", "、", "．"] {
            if let Some(r) = rest.strip_prefix(m) {
                return r;
            }
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LlmProvider;
    use std::sync::Mutex;

    #[test]
    fn parse_steps_strips_markers_and_noise() {
        let raw = "以下是拆解：\n\
                   - 在 render::integrate 加 --adaptive flag\n\
                   2. 写单测覆盖新分支\n\
                   * 更新 CHANGELOG\n\
                   \n   \n";
        assert_eq!(
            parse_steps(raw),
            vec![
                "在 render::integrate 加 --adaptive flag",
                "写单测覆盖新分支",
                "更新 CHANGELOG",
            ]
        );
    }

    /// A canned provider that returns a fixed list and records the prompt it saw, so we
    /// can assert breakdown wires the task + context into the message and parses output.
    struct Mock {
        reply: String,
        last_user: Mutex<Option<String>>,
    }
    impl LlmProvider for Mock {
        async fn complete(&self, _system: &str, user: &str) -> anyhow::Result<String> {
            *self.last_user.lock().unwrap() = Some(user.to_string());
            Ok(self.reply.clone())
        }
    }

    #[tokio::test]
    async fn breakdown_wires_context_and_parses_candidates() {
        let mock = Mock {
            reply: "- step one\n- step two\n- step three\n".to_string(),
            last_user: Mutex::new(None),
        };
        let steps = breakdown("wire the sampler", Some("blocked on denoiser"), &mock)
            .await
            .unwrap();
        assert_eq!(steps, vec!["step one", "step two", "step three"]);
        let user = mock.last_user.lock().unwrap().clone().unwrap();
        assert!(user.contains("wire the sampler"), "task injected");
        assert!(user.contains("blocked on denoiser"), "context injected");
    }
}
