//! LLM provider adapters (spec §0 / §7; charter §5 原则8 provider-neutral).
//!
//! Two wire formats cover the field:
//!  - `AnthropicProvider` — native Anthropic `/v1/messages`
//!  - `OpenAiProvider`    — OpenAI **`/v1/responses`** (the current reasoning-model
//!    surface): OpenAI, OpenRouter, DeepSeek, Ollama, and any endpoint that speaks
//!    Responses via base_url. OmniProj went all-in on Responses (dropping the legacy
//!    `/chat/completions` path) — endpoints that don't expose `/v1/responses` (e.g.
//!    Groq/xAI/Together/Gemini as of 2026-08, unverified) are surfaced by `omniproj
//!    doctor` before a call is made rather than failing mid-distill.
//!
//! `AnyProvider` is the runtime dispatch enum the resolver (config.rs) builds.
//!
//! **Reasoning effort** (`Tuning::effort`) is a neutral level (`low`..`max`) each
//! adapter translates to its own wire shape: Anthropic → `output_config.effort`,
//! Responses → `reasoning.effort`. Only sent when configured, so an un-tuned call is
//! byte-identical to the pre-effort request.
//!
//! **Token budget** (`Tuning::max_output_tokens`) is a COMBINED cap on reasoning +
//! visible output on reasoning models — verified against DeepSeek: a 2-char answer at
//! effort=low still spent 57 reasoning tokens. Since every OpenAI call now goes through
//! Responses (where the model reasons within the budget), the default is raised from the
//! historical 4096. Too tight a cap is consumed entirely by reasoning and returns
//! `status: "incomplete"` with an empty message — which the adapter detects and fails
//! loudly rather than writing a blank distill.
//!
//! Reliability (benchmark review P1#4): every call has a hard timeout — reqwest's
//! default is NONE, which once let a hung endpoint stall the daemon's single worker
//! forever — and transient failures (transport errors, 429/5xx) are retried with
//! exponential backoff. Non-transient API errors (4xx, empty completion) fail fast.

use std::time::Duration;

/// Default output-token cap when a caller doesn't override it. Raised from the historical
/// 4096 because every OpenAI call now goes through Responses, where the model spends part
/// of this COMBINED budget on reasoning before any visible output (see [`Tuning`]). A cap,
/// not a target — Anthropic non-reasoning models simply don't approach it.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 16_000;

/// Per-call tuning threaded from config into a provider at construction. Default =
/// no effort + the historical token cap, so an un-tuned provider behaves exactly as
/// before this knob existed.
#[derive(Debug, Clone)]
pub struct Tuning {
    /// Neutral reasoning-effort level (`low`/`medium`/`high`/`xhigh`/`max`). `None`
    /// omits the parameter entirely. Adapters translate; unsupported wire formats warn.
    pub effort: Option<String>,
    /// Output-token cap. A COMBINED budget (reasoning + text) on reasoning models.
    pub max_output_tokens: u32,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            effort: None,
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
        }
    }
}
/// Hard cap per HTTP attempt. Deep-pipeline calls are large but bounded; anything
/// slower than this is treated as a hung endpoint.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total attempts = 1 + 2 retries.
const ATTEMPTS: u32 = 3;
/// Backoff before retry n (1-based): BASE * 4^(n-1) → 1s, 4s.
const BACKOFF_BASE: Duration = Duration::from_secs(1);

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .expect("static client config cannot fail")
}

/// Classification of a failed attempt: transient failures retry, the rest fail fast.
enum CallError {
    /// Transport error / timeout / 429 / 5xx — the request may succeed if repeated.
    Retryable(anyhow::Error),
    /// Auth errors, malformed requests, empty completions — repeating won't help.
    Fatal(anyhow::Error),
}

/// Should an HTTP status be retried? 429 (rate limit) + all 5xx.
fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

fn backoff_delay(retry_n: u32, base: Duration) -> Duration {
    base * 4u32.saturating_pow(retry_n - 1)
}

/// Run `attempt` up to [`ATTEMPTS`] times, sleeping [`backoff_delay`] between tries.
/// `base` is injectable so tests run with Duration::ZERO.
async fn with_retry<T, F, Fut>(mut attempt: F, base: Duration) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, CallError>>,
{
    let mut last: Option<anyhow::Error> = None;
    for n in 1..=ATTEMPTS {
        match attempt().await {
            Ok(v) => return Ok(v),
            Err(CallError::Fatal(e)) => return Err(e),
            Err(CallError::Retryable(e)) => {
                last = Some(e);
                if n < ATTEMPTS {
                    tokio::time::sleep(backoff_delay(n, base)).await;
                }
            }
        }
    }
    Err(last.expect("loop ran at least once").context(format!(
        "LLM call failed after {ATTEMPTS} attempts (transient errors, retried with backoff)"
    )))
}

#[allow(async_fn_in_trait)]
pub trait LlmProvider {
    /// One-shot completion: system prompt + a single user message -> text.
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String>;
}

// ----------------------------------------------------------------------------- Anthropic

pub struct AnthropicProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    tuning: Tuning,
}

impl AnthropicProvider {
    pub fn new(base_url: String, api_key: String, model: String, tuning: Tuning) -> Self {
        Self {
            client: http_client(),
            base_url,
            api_key,
            model,
            tuning,
        }
    }

    async fn try_complete(&self, system: &str, user: &str) -> Result<String, CallError> {
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.tuning.max_output_tokens,
            "system": system,
            "messages": [{ "role": "user", "content": user }],
        });
        // Reasoning effort → Anthropic `output_config.effort` (GA on 4.6+ models).
        // Only sent when configured, so an un-tuned request is unchanged.
        if let Some(effort) = &self.tuning.effort {
            body["output_config"] = serde_json::json!({ "effort": effort });
        }
        let resp = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CallError::Retryable(e.into()))?;
        let status = resp.status();
        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CallError::Retryable(e.into()))?;
        if !status.is_success() {
            let err = anyhow::anyhow!("Anthropic API error {status}: {val}");
            return Err(if is_retryable_status(status.as_u16()) {
                CallError::Retryable(err)
            } else {
                CallError::Fatal(err)
            });
        }
        let text = val
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Err(CallError::Fatal(anyhow::anyhow!("empty completion: {val}")));
        }
        Ok(text)
    }
}

impl LlmProvider for AnthropicProvider {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        with_retry(|| self.try_complete(system, user), BACKOFF_BASE).await
    }
}

// ---------------------------------------------------------------- OpenAI (Responses API)

pub struct OpenAiProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    tuning: Tuning,
}

/// The three shapes a `/v1/responses` body resolves to, separated so parsing is pure
/// and unit-testable against real captured payloads.
#[derive(Debug, PartialEq)]
enum ResponsesOutcome {
    /// A non-empty assistant message.
    Text(String),
    /// `status: "incomplete"` with no message text — the token budget was consumed
    /// (usually entirely by reasoning). Carries the API's `reason`. Raising
    /// `max_output_tokens` is the fix; a plain retry would repeat the truncation.
    Incomplete(String),
    /// 200 with no message text and not flagged incomplete — treat as a failed call.
    Empty,
}

/// Extract the assistant text from a Responses payload. The visible answer lives in
/// `output[]` items of `type == "message"`, whose `content[]` parts carry `text`
/// (reasoning items have no text and are skipped). `output_text` is an SDK convenience
/// field, NOT present in raw HTTP — so we walk `output` ourselves.
fn parse_responses(val: &serde_json::Value) -> ResponsesOutcome {
    let text: String = val
        .get("output")
        .and_then(|o| o.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|it| it.get("type").and_then(|t| t.as_str()) == Some("message"))
                .filter_map(|it| it.get("content").and_then(|c| c.as_array()))
                .flatten()
                .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    if !text.trim().is_empty() {
        return ResponsesOutcome::Text(text);
    }
    if val.get("status").and_then(|s| s.as_str()) == Some("incomplete") {
        let reason = val
            .get("incomplete_details")
            .and_then(|d| d.get("reason"))
            .and_then(|r| r.as_str())
            .unwrap_or("unknown")
            .to_string();
        return ResponsesOutcome::Incomplete(reason);
    }
    ResponsesOutcome::Empty
}

impl OpenAiProvider {
    pub fn new(base_url: String, api_key: String, model: String, tuning: Tuning) -> Self {
        Self {
            client: http_client(),
            base_url,
            api_key,
            model,
            tuning,
        }
    }

    async fn try_complete(&self, system: &str, user: &str) -> Result<String, CallError> {
        // Responses shape: system → `instructions`, user → `input`,
        // token cap → `max_output_tokens`, effort → `reasoning.effort`.
        let mut body = serde_json::json!({
            "model": self.model,
            "instructions": system,
            "input": user,
            "max_output_tokens": self.tuning.max_output_tokens,
        });
        if let Some(effort) = &self.tuning.effort {
            body["reasoning"] = serde_json::json!({ "effort": effort });
        }
        let mut req = self
            .client
            .post(format!("{}/responses", self.base_url.trim_end_matches('/')))
            .header("content-type", "application/json")
            .json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| CallError::Retryable(e.into()))?;
        let status = resp.status();
        let val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CallError::Retryable(e.into()))?;
        if !status.is_success() {
            let err = anyhow::anyhow!("Responses API error {status}: {val}");
            return Err(if is_retryable_status(status.as_u16()) {
                CallError::Retryable(err)
            } else {
                CallError::Fatal(err)
            });
        }
        match parse_responses(&val) {
            ResponsesOutcome::Text(t) => Ok(t),
            // Incomplete/Empty are fatal: retrying the identical request repeats the
            // truncation. The message tells the user the actionable fix.
            ResponsesOutcome::Incomplete(reason) => Err(CallError::Fatal(anyhow::anyhow!(
                "response truncated (status=incomplete, reason={reason}): the token budget \
                 was consumed before any visible output — raise max_output_tokens \
                 (currently {}). This model reasons within that budget.",
                self.tuning.max_output_tokens
            ))),
            ResponsesOutcome::Empty => Err(CallError::Fatal(anyhow::anyhow!(
                "empty completion from Responses API: {val}"
            ))),
        }
    }
}

impl LlmProvider for OpenAiProvider {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        with_retry(|| self.try_complete(system, user), BACKOFF_BASE).await
    }
}

// ------------------------------------------------------------------------------- dispatch

pub enum AnyProvider {
    Anthropic(AnthropicProvider),
    OpenAi(OpenAiProvider),
}

impl LlmProvider for AnyProvider {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        match self {
            AnyProvider::Anthropic(p) => p.complete(system, user).await,
            AnyProvider::OpenAi(p) => p.complete(system, user).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn status_classification() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
    }

    #[test]
    fn backoff_grows_exponentially() {
        let base = Duration::from_secs(1);
        assert_eq!(backoff_delay(1, base), Duration::from_secs(1));
        assert_eq!(backoff_delay(2, base), Duration::from_secs(4));
    }

    #[tokio::test]
    async fn retries_transient_then_succeeds() {
        let calls = AtomicU32::new(0);
        let out = with_retry(
            || {
                let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if n < 3 {
                        Err(CallError::Retryable(anyhow::anyhow!("transient {n}")))
                    } else {
                        Ok("ok")
                    }
                }
            },
            Duration::ZERO,
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn fatal_fails_immediately_without_retry() {
        let calls = AtomicU32::new(0);
        let out: anyhow::Result<&str> = with_retry(
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Err(CallError::Fatal(anyhow::anyhow!("bad key"))) }
            },
            Duration::ZERO,
        )
        .await;
        assert!(out.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "fatal must not retry");
    }

    #[tokio::test]
    async fn exhausted_retries_surface_the_last_error() {
        let out: anyhow::Result<&str> = with_retry(
            || async { Err(CallError::Retryable(anyhow::anyhow!("still down"))) },
            Duration::ZERO,
        )
        .await;
        let msg = format!("{:#}", out.unwrap_err());
        assert!(msg.contains("after 3 attempts"), "got: {msg}");
        assert!(msg.contains("still down"));
    }

    // --- Responses API parsing (real payloads captured from DeepSeek 2026-08-09) ---

    #[test]
    fn parse_responses_extracts_message_text_past_reasoning_item() {
        // Real shape: a reasoning item precedes the message item; only message text counts.
        let val = serde_json::json!({
            "model": "deepseek-v4-flash",
            "status": "completed",
            "output": [
                { "type": "reasoning", "summary": [] },
                { "type": "message", "content": [{ "type": "output_text", "text": "OK" }] }
            ],
            "usage": { "output_tokens": 59, "output_tokens_details": { "reasoning_tokens": 57 } }
        });
        assert_eq!(parse_responses(&val), ResponsesOutcome::Text("OK".into()));
    }

    #[test]
    fn parse_responses_flags_budget_exhaustion_as_incomplete() {
        // Real captured failure: max_output_tokens=24 consumed entirely by reasoning,
        // output has ONLY a reasoning item, no message. HTTP was 200 with no error.
        let val = serde_json::json!({
            "status": "incomplete",
            "incomplete_details": { "reason": "max_output_tokens" },
            "output": [ { "type": "reasoning", "summary": [] } ],
            "usage": { "output_tokens": 24, "output_tokens_details": { "reasoning_tokens": 24 } }
        });
        assert_eq!(
            parse_responses(&val),
            ResponsesOutcome::Incomplete("max_output_tokens".into())
        );
    }

    #[test]
    fn parse_responses_empty_when_no_message_and_not_incomplete() {
        let val = serde_json::json!({ "status": "completed", "output": [] });
        assert_eq!(parse_responses(&val), ResponsesOutcome::Empty);
    }

    #[test]
    fn parse_responses_joins_multiple_message_parts() {
        let val = serde_json::json!({
            "status": "completed",
            "output": [ { "type": "message", "content": [
                { "type": "output_text", "text": "part1 " },
                { "type": "output_text", "text": "part2" }
            ] } ]
        });
        assert_eq!(
            parse_responses(&val),
            ResponsesOutcome::Text("part1 part2".into())
        );
    }

    #[test]
    fn tuning_default_is_no_effort_at_the_default_cap() {
        // An un-tuned provider sends no effort and uses the default budget (raised to
        // leave reasoning headroom now that every OpenAI call goes through Responses).
        let t = Tuning::default();
        assert_eq!(t.max_output_tokens, DEFAULT_MAX_OUTPUT_TOKENS);
        assert_eq!(DEFAULT_MAX_OUTPUT_TOKENS, 16_000);
        assert!(t.effort.is_none());
    }
}
