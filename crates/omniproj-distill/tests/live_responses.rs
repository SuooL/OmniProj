//! Live smoke test for the Responses-API provider path (cockpit P2a).
//!
//! `#[ignore]`d so it never runs in CI (no network, no keys there). Run explicitly
//! against a real endpoint to prove the `openai_responses` kind + effort + the
//! incomplete-budget guard work end-to-end:
//!
//! ```sh
//! OMNIPROJ_HOME=$(mktemp -d) DEEPSEEK_API_KEY=... \
//!   cargo test -p omniproj-distill --test live_responses -- --ignored --test-threads=1
//! ```
//! Both tests write config.toml into the same `OMNIPROJ_HOME`, so `--test-threads=1` is
//! required — parallel runs race on the shared config file.

use omniproj_distill::config;
use omniproj_distill::provider::LlmProvider;

fn write_config(home: &str, body: &str) {
    std::fs::write(format!("{home}/config.toml"), body).unwrap();
}

#[tokio::test]
#[ignore = "hits a live provider; run manually with DEEPSEEK_API_KEY set"]
async fn deepseek_responses_completes_with_effort() {
    let home = std::env::var("OMNIPROJ_HOME").expect("set OMNIPROJ_HOME to an isolated dir");
    write_config(
        &home,
        "default_model = \"deepseek/deepseek-chat\"\n\
         [clarify]\n\
         model = \"deepseek/deepseek-chat\"\n\
         effort = \"low\"\n\
         max_output_tokens = 8000\n",
    );
    let resolved = config::resolve_clarify().expect("resolve clarify");
    assert_eq!(resolved.provider_name, "deepseek");
    let out = resolved
        .provider
        .complete("Reply with exactly the word: PONG", "ping")
        .await
        .expect("live completion should succeed");
    assert!(
        out.to_uppercase().contains("PONG"),
        "expected the model to echo PONG, got: {out:?}"
    );
}

#[tokio::test]
#[ignore = "hits a live provider; run manually with DEEPSEEK_API_KEY set"]
async fn tight_budget_surfaces_incomplete_not_blank() {
    // A budget so small it's consumed by reasoning must FAIL LOUDLY, never return a
    // blank string that a distill would then write over the user's state.
    let home = std::env::var("OMNIPROJ_HOME").expect("set OMNIPROJ_HOME to an isolated dir");
    write_config(
        &home,
        "default_model = \"deepseek/deepseek-chat\"\n\
         [clarify]\n\
         model = \"deepseek/deepseek-chat\"\n\
         effort = \"low\"\n\
         max_output_tokens = 16\n",
    );
    let resolved = config::resolve_clarify().expect("resolve clarify");
    let err = resolved
        .provider
        .complete("Explain in one sentence why the sky is blue.", "go")
        .await
        .expect_err("a 16-token budget must not yield a successful blank completion");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("incomplete") || msg.contains("truncated"),
        "error must name the truncation so the user knows to raise the budget, got: {msg}"
    );
}
