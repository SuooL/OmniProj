//! Advance — idea refinement (推进层, FR-V2). Turn a rough, not-yet-clear idea into a
//! clear, testable requirement/spec, grounded in the repo's context. Human-in-the-loop:
//! the spec is AI derivative (lands in `auto/`), the user decides what to do with it.
//!
//! Note: the charter's FR-V2 also allows *web* research; that needs a browsing-capable
//! provider/tool and is not wired here. This is the repo-grounded half — the model is given
//! the idea + recent git context, no live web.

use crate::provider::LlmProvider;

pub const REFINE_SYSTEM_PROMPT: &str = r#"你是一个把「模糊的想法」打磨成「清晰、可验收的需求/spec」的助手。用户给你一条还很粗的想法,外加一点仓库上下文(分支、近期提交)。你的职责是把它写成一份简短的 spec,让它从「想不清」变成「知道做完长什么样」。

输出结构(markdown,简洁,中文除非语境是英文):
## 目标
一句话:做完之后达成什么(用户视角,不是实现视角)。
## 范围
要做的 / 明确不做的(各 1-3 条)。
## 验收标准
2-4 条**可判定**的条件(能回答「做完了吗」的 yes/no,尽量能对应到测试/提交/可观察行为)。
## 待定
1-3 个还需用户拍板的开放问题(不要替他决定)。

硬规则:只产出 spec,不写实现、不写代码、不推荐技术选型、不替用户拍板。基于给定上下文,不要编造仓库里没有的事实。"#;

/// Refine an idea into a spec (FR-V2). `context` is optional repo grounding (branch +
/// recent commit subjects). Returns the model's spec markdown verbatim.
pub async fn refine(
    idea: &str,
    context: Option<&str>,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let ctx = match context {
        Some(c) if !c.trim().is_empty() => format!("\n## 仓库上下文\n{}\n", c.trim()),
        _ => String::new(),
    };
    let msg = format!("## 待打磨的想法\n{}\n{ctx}", idea.trim());
    let out = provider.complete(REFINE_SYSTEM_PROMPT, &msg).await?;
    Ok(out.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LlmProvider;
    use std::sync::Mutex;

    struct Mock {
        reply: String,
        last_user: Mutex<Option<String>>,
    }
    impl LlmProvider for Mock {
        async fn complete(&self, _s: &str, user: &str) -> anyhow::Result<String> {
            *self.last_user.lock().unwrap() = Some(user.to_string());
            Ok(self.reply.clone())
        }
    }

    #[tokio::test]
    async fn refine_injects_idea_and_context() {
        let mock = Mock {
            reply: "## 目标\n…\n".to_string(),
            last_user: Mutex::new(None),
        };
        let out = refine(
            "add adaptive sampling",
            Some("branch: feat/x\n- earlier commit"),
            &mock,
        )
        .await
        .unwrap();
        assert!(out.starts_with("## 目标"));
        let u = mock.last_user.lock().unwrap().clone().unwrap();
        assert!(u.contains("add adaptive sampling"));
        assert!(u.contains("branch: feat/x"));
    }
}
