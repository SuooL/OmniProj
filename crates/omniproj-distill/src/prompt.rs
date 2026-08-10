//! Distillation prompt (spec §5.1; full IP in `spike/distill-prompt.md`).
//! Stress-tested on 3 real projects before being committed to code.

pub const SYSTEM_PROMPT: &str = r#"你是某个项目的「认知状态蒸馏器」。目标:让用户隔了数天/数周回到这个项目时,<5 分钟重新上手。你只呈现状态,不做决策——不排优先级、不催进度、不替用户判断该做什么。

输入顶部是一份 **VERIFIED FACTS**(已由代码核验的 git 事实白名单),其下是 substrate digest:git 实况(branch / HEAD / status / 近期 commit / diffstat)+ 最近若干 session 的归一对话(user/assistant)。

硬规则:
1. live-git 是「当前状态」的权威。briefing 顶部 pin branch + live HEAD 短 hash + 一句 freshness 声明(反映当前 git,非快照)。若 session 自述与 git 矛盾,以 git 为准。
2. 无 git 时,状态据「最新 session + 文件」,并注明「无 git」。
3. 参考文档(若 digest/对话里出现 HANDOFF/STATUS/README)只是「带新鲜度标注的参考」,不是真相源:可链接、可引用,但要对照 git 校验、标注其可能过期/阶段性,绝不照搬为当前状态。
4. re-entry 点分态:暂停态→突出「为何停 + 解锁条件 + 回来第一件事」;活跃态(最近仍在 commit)→突出「当前前沿 + 刚完成/刚解锁 + 回来第一件事」,不要硬套「为何暂停」。
5. 过滤与项目无关的内容(跑题会话、注入的 skill/AGENTS 文本)。
6. 「卡在哪」尽量用 session 里 tool 调用的真实结果/退出码兜底,而非 agent 自述。
7. 只在 session 里、不在 git 的数字(如测试数)标注来源;任何推断标「AI 推断」。绝不编造 commit / 数字 / 文件名。凡写出的 commit 短 hash **必须能在 VERIFIED FACTS 的白名单里找到前缀**;找不到就写「未知」或不写,绝不杜撰(代码会逐个核验并标注未核实项)。
8. decisions 用「已做的判断/结论」,带日期;open 用「未闭环卡点/待决」,尽量给「回来第一件事」。
9. 简洁:briefing 读完应 < 60 秒。绝不打分、不催、不排优先级。
10. 若输入含 LEARNED(本项目历史修正得出的呈现/抽取偏好),**遵循它**——它代表用户已纠正过的口味(只影响怎么呈现,不改变事实)。
11. 若输入含 USER MODEL(用户画像维度,spec §4.4),把它当**呈现透镜**:domain_prior 里已熟悉的概念不必解释、presentation_pref 决定详略与格式、其余维度帮助判断哪些状态对用户重要。它**只影响怎么讲,绝不改变事实本身**,也不得据此替用户做决策。

输出格式:严格输出以下三段,用分隔行界定,段内是 markdown,不要额外解释:
===BRIEFING===
<briefing.md 内容>
===DECISIONS===
<decisions.md 内容>
===OPEN===
<open.md 内容>
"#;

use omniproj_core::FactSheet;

pub fn user_message(digest: &str, facts: &FactSheet, learned: &str, user_model: &str) -> String {
    format!(
        "下面是这个项目的 VERIFIED FACTS{}与 substrate digest。请据此产出 briefing / decisions / open 三段。\n\n{}{}{}\n{digest}",
        if learned.trim().is_empty() { " " } else { "、LEARNED 偏好 " },
        render_facts(facts),
        render_learned(learned),
        render_user_model(user_model),
    )
}

/// Enabled user-model dimensions (spec §4.4), rendered by core. Empty -> no block.
fn render_user_model(user_model: &str) -> String {
    if user_model.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n## USER MODEL(用户画像,呈现透镜——只影响怎么讲,不改变事实)\n{}\n",
            user_model.trim()
        )
    }
}

/// Per-project heuristics from past user corrections (spec §5.3). Empty -> no block.
fn render_learned(learned: &str) -> String {
    if learned.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n## LEARNED(本项目历史修正得出的呈现/抽取偏好,请遵循)\n{}\n",
            learned.trim()
        )
    }
}

/// The authoritative, code-verified fact block injected ahead of the digest.
/// The model is told to cite commits only from here (spec §5.2 grounded prompting).
/// `pub(crate)`: the second-opinion pass (spec §4.5) grounds on the same block.
pub(crate) fn render_facts(facts: &FactSheet) -> String {
    match &facts.git {
        Some(g) => {
            let shorts: Vec<String> = g
                .commit_hashes
                .iter()
                .take(40)
                .map(|h| h.chars().take(8).collect::<String>())
                .collect();
            format!(
                "## VERIFIED FACTS(只能引用此处的 commit;引不到就写「未知」,绝不编造)\n\
                 branch: {}\nHEAD: {}\n已知 commit 短 hash 白名单:\n  {}\n",
                g.branch,
                g.head_short,
                shorts.join(" ")
            )
        }
        None => "## VERIFIED FACTS\n(无 git —— 不要写任何 commit hash;状态据最新 session/文件)\n"
            .to_string(),
    }
}
