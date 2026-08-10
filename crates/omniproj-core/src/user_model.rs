//! User Model (spec §4.4, charter §5 原则7) — a domain-specific, user-editable,
//! per-dimension disable-able profile at `~/.omniproj/user/model.md`.
//!
//! Plain markdown is the storage format on purpose: the user can read/edit/disable
//! everything with any editor (charter: 用户能看到、能修改、能禁用). A dimension is
//! disabled by appending `(disabled)` to its `## ` heading — no hidden state.
//!
//! Consumers: distillation injects the *enabled* dimensions as presentation/lens
//! preferences; second opinion (spec §4.5) deliberately ignores chosen dimensions —
//! that's why the parse keeps dimensions separate rather than treating the file as
//! one blob.

use std::path::PathBuf;

use crate::paths::omniproj_home;

/// The v1 dimension vocabulary (spec §4.4). Fixed set; schema swap is a future hook.
pub const DIMENSIONS: [&str; 5] = [
    "domain_prior",
    "methodology_pref",
    "risk_pref",
    "mainline_vs_sidebet",
    "presentation_pref",
];

/// Per-dimension user-model budget (chars). Over it, surfaces warn the user but
/// never rewrite the model file because it is user-owned.
pub const USER_MODEL_DIM_CAP_CHARS: usize = 2_000;

/// `~/.omniproj/user/model.md`
pub fn user_model_path() -> PathBuf {
    omniproj_home().join("user").join("model.md")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    /// One of [`DIMENSIONS`] (unknown headings are kept too — user-extensible).
    pub name: String,
    pub enabled: bool,
    /// Body text under the heading (trimmed; may be empty if未填写).
    pub body: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserModel {
    pub dimensions: Vec<Dimension>,
}

impl UserModel {
    /// Parse the markdown model. Sections start at `## <name>`; a trailing
    /// `(disabled)` on the heading line disables that dimension. Content before the
    /// first heading is ignored (file preamble).
    pub fn parse(text: &str) -> Self {
        let mut dims: Vec<Dimension> = Vec::new();
        let mut cur: Option<Dimension> = None;
        for line in text.lines() {
            if let Some(h) = line.strip_prefix("## ") {
                if let Some(d) = cur.take() {
                    dims.push(d);
                }
                let h = h.trim();
                let (name, enabled) = match h.strip_suffix("(disabled)") {
                    Some(n) => (n.trim(), false),
                    None => (h, true),
                };
                cur = Some(Dimension {
                    name: name.to_string(),
                    enabled,
                    body: String::new(),
                });
            } else if let Some(d) = cur.as_mut() {
                d.body.push_str(line);
                d.body.push('\n');
            }
        }
        if let Some(d) = cur.take() {
            dims.push(d);
        }
        for d in &mut dims {
            d.body = strip_html_comments(&d.body).trim().to_string();
        }
        UserModel { dimensions: dims }
    }

    /// Load from `~/.omniproj/user/model.md`. Missing file → empty model (feature off).
    pub fn load() -> Self {
        match std::fs::read_to_string(user_model_path()) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }

    /// Enabled, non-empty dimensions — what distillation may see.
    pub fn enabled(&self) -> impl Iterator<Item = &Dimension> {
        self.dimensions
            .iter()
            .filter(|d| d.enabled && !d.body.is_empty())
    }

    /// Render the enabled dimensions as a prompt block, excluding `ignore`d names
    /// (second opinion's counter-convergence hook, spec §4.5). Empty string when
    /// nothing applies — callers skip the block entirely.
    pub fn render_for_prompt(&self, ignore: &[&str]) -> String {
        let parts: Vec<String> = self
            .enabled()
            .filter(|d| !ignore.contains(&d.name.as_str()))
            .map(|d| format!("### {}\n{}", d.name, d.body))
            .collect();
        if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n\n")
        }
    }
}

/// Drop `<!-- … -->` spans so the template's placeholder hints don't count as
/// content — a freshly `--init`ed model must be inert until the user writes into it.
fn strip_html_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out, // unclosed comment swallows the remainder
        }
    }
    out.push_str(rest);
    out
}

/// Starter template written by `omniproj model --init`. All dimensions present but
/// empty; the user fills in what they want, deletes or `(disabled)`-marks the rest.
pub const USER_MODEL_TEMPLATE: &str = r#"# OmniProj User Model

这是你的画像文件(spec §4.4)。蒸馏会参考**启用且非空**的维度来调整呈现;
second opinion 会刻意忽略其中一些以保持「不像你」的对照视角。
- 随时编辑;留空的维度不生效。
- 在标题后加 `(disabled)` 可单独禁用某维度,如 `## risk_pref (disabled)`。

## domain_prior
<!-- 领域先验/已知背景:你熟悉什么,briefing 里哪些概念不用解释 -->

## methodology_pref
<!-- 方法学偏好:你偏好的研究/工程方法,如何评估证据 -->

## risk_pref
<!-- 风险偏好:激进尝试 vs 保守推进;对不确定性的容忍度 -->

## mainline_vs_sidebet
<!-- 主线 vs side bet:哪些项目/方向是主线,哪些是赌注性的支线 -->

## presentation_pref
<!-- 呈现偏好:详略、术语密度、格式(列表/散文)、语言 -->
"#;

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# preamble ignored\n\n## domain_prior\nRust + LLM tooling.\n\n## risk_pref (disabled)\nconservative\n\n## presentation_pref\n简洁,列表优先。\n\n## custom_dim\nuser-added\n";

    #[test]
    fn parses_sections_and_disabled_marker() {
        let m = UserModel::parse(SAMPLE);
        assert_eq!(m.dimensions.len(), 4);
        let risk = m.dimensions.iter().find(|d| d.name == "risk_pref").unwrap();
        assert!(!risk.enabled);
        assert_eq!(risk.body, "conservative");
        let domain = m
            .dimensions
            .iter()
            .find(|d| d.name == "domain_prior")
            .unwrap();
        assert!(domain.enabled);
    }

    #[test]
    fn enabled_skips_disabled_and_empty() {
        let m = UserModel::parse("## a\ncontent\n\n## b (disabled)\nx\n\n## c\n\n");
        let names: Vec<&str> = m.enabled().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["a"]); // b disabled, c empty
    }

    #[test]
    fn render_excludes_ignored_dims() {
        let m = UserModel::parse(SAMPLE);
        let all = m.render_for_prompt(&[]);
        assert!(all.contains("domain_prior") && all.contains("presentation_pref"));
        assert!(!all.contains("risk_pref")); // disabled
        let ignored = m.render_for_prompt(&["presentation_pref"]);
        assert!(!ignored.contains("presentation_pref"));
        assert!(ignored.contains("domain_prior"));
    }

    #[test]
    fn empty_model_renders_empty() {
        assert_eq!(UserModel::default().render_for_prompt(&[]), "");
    }

    #[test]
    fn template_parses_with_all_dims_empty() {
        let m = UserModel::parse(USER_MODEL_TEMPLATE);
        assert_eq!(m.dimensions.len(), DIMENSIONS.len());
        // bodies are only HTML-comment placeholders → stripped → inert until filled
        assert!(m.enabled().next().is_none(), "fresh template must be inert");
    }
}
