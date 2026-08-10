//! Second Opinion — the explicit anti-convergence pass (spec §4.5, charter §5 原则5).
//!
//! The value comes from being deliberately UNLIKE the user: the chosen user-model
//! dimensions are withheld from the prompt and *named* as ignored axes, so the model
//! knows where to diverge instead of hoping divergence emerges naturally. Output is
//! 「标记 + 理由」 — it challenges the convergent view (briefing/decisions/open), it
//! never recommends or executes (charter §5 原则3).

use crate::provider::LlmProvider;
use omniproj_core::FactSheet;

pub const OPINION_SYSTEM_PROMPT: &str = r#"你是项目状态的「second opinion」——一个刻意反收敛的对照视角。输入是:已有的蒸馏结论(briefing/decisions/open,即「主流视角」)、VERIFIED FACTS(代码核验过的 git 事实)、substrate digest,以及一份说明:哪些用户画像维度被**刻意忽略**了。

你的任务:站在「不像这个用户」的立场,挑战主流视角。被忽略的维度正是你要偏离的轴——例如忽略 risk_pref 就请用相反的风险姿态审视;忽略 mainline_vs_sidebet 就请质疑主线/支线的划分本身。

硬规则:
1. 输出是「标记 + 理由」清单,不是建议清单。每条 = **标记**(主流视角里哪条结论/盲区被挑战) + **理由**(基于事实的另一种解读)。绝不说「你应该/建议你」——只呈现被忽略的角度,决策权在用户。
2. 所有 commit hash 必须出自 VERIFIED FACTS 白名单;引不到就不引,绝不编造。
3. 挑战要有事实根据(git/digest 里找得到),不是为反而反;找不到可挑战的就少写,宁缺毋滥。
4. 3-6 条为宜,每条 2-4 行。简洁,中文。
5. 顶部一行说明:本视角刻意忽略了哪些维度(原样列出)。

输出 markdown,无需分隔标记。"#;

/// Inputs to a second-opinion pass, grouped to keep the call site legible.
///
/// `ignored_dims` are the user-model dimension names deliberately withheld (may be
/// empty when no model exists — still a valid contrast pass, just unpersonalized);
/// `kept_model` is the rendered remainder.
pub struct OpinionInput<'a> {
    pub briefing: &'a str,
    pub decisions: &'a str,
    pub open: &'a str,
    pub digest: &'a str,
    pub facts: &'a FactSheet,
    pub ignored_dims: &'a [String],
    pub kept_model: &'a str,
}

/// Run the second-opinion pass.
pub async fn second_opinion(
    input: &OpinionInput<'_>,
    provider: &impl LlmProvider,
) -> anyhow::Result<String> {
    let OpinionInput {
        briefing,
        decisions,
        open,
        digest,
        facts,
        ignored_dims,
        kept_model,
    } = *input;
    let ignored = if ignored_dims.is_empty() {
        "(无用户画像可忽略——本视角为无个性化的通用对照)".to_string()
    } else {
        ignored_dims.join(", ")
    };
    let kept = if kept_model.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n## 保留的画像维度(可参考,但别被它收编)\n{}\n",
            kept_model.trim()
        )
    };
    let msg = format!(
        "## 刻意忽略的画像维度\n{ignored}\n{kept}\n## 主流视角(待挑战)\n### briefing\n{briefing}\n\n### decisions\n{decisions}\n\n### open\n{open}\n\n{facts_block}\n{digest}",
        facts_block = crate::prompt::render_facts(facts),
    );
    provider.complete(OPINION_SYSTEM_PROMPT, &msg).await
}
