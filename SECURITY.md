# Security Policy

## Reporting a vulnerability

Please report security vulnerabilities **privately**, not via a public issue.

- Use GitHub's [private vulnerability reporting](https://github.com/SuooL/OmniProj/security/advisories/new)
  ("Report a vulnerability" under the repository's **Security** tab), or
- open a minimal public issue that says only "security report — please provide a
  private contact" if private reporting is unavailable.

Please include what you can reproduce, the affected version (`omniproj --version`), and
your OS. We aim to acknowledge reports within a few days. There is no bug-bounty
program; this is a small open-source project.

## Data boundary (read this)

OmniProj is local-first, but **distillation is not fully offline unless you choose a
local model.** Understanding exactly what leaves your machine is part of using it
safely.

- **Persistent state is local.** Everything OmniProj stores lives in `~/.omniproj`
  (markdown + a git history). It is never uploaded.
- **Distillation sends a digest to your configured LLM provider.** To produce a
  briefing, OmniProj assembles a *digest* — text derived from your git activity and your
  Claude Code / Codex session transcripts — and sends it to whichever provider you
  configured (Anthropic, OpenAI, DeepSeek, OpenRouter, etc.). That request leaves
  your machine and is subject to that provider's data policies.
- **Treat captured sessions as potentially sensitive.** Session transcripts can
  contain secrets you pasted, private code, or confidential project context. The
  target audience (researchers/developers) routinely has sensitive data.

### What OmniProj does to reduce exposure (on by default)

- **Sensitive paths are dropped.** Paths matching a deny-list (`.env*`, `*.key`,
  `*.pem`, `id_rsa*`, `secrets/`, `credentials*`, `*.p12`, …) are removed from the
  outbound digest. The deny-list is configurable via `[privacy] deny_globs`.
- **Secret shapes are masked.** Common credential patterns (`sk-…`, `AKIA…`,
  `Bearer …`, `KEY=value`, PEM headers, …) are masked before sending. This can be
  disabled per run with `--no-redact` (the deny-list still applies).
- **Preview before sending.** `omniproj digest` prints the **exact** text that would be
  sent to the provider, so you can inspect it first.
- **Consent notice.** The CLI reminds you once (until you set
  `[privacy] send_consent = true`) that the digest goes to a remote provider.

### The fully-local path

For a nothing-leaves-the-machine setup, point `default_model` at a local
[Ollama](https://ollama.com) model (e.g. `default_model = "ollama/llama3.1"`). Then no
digest is sent to any third party. This is the recommended path for sensitive work.

> Note: automatic secret detection is best-effort, not a guarantee. Do not rely on it
> as your only safeguard for highly sensitive data — prefer a local model, and use
> `omniproj digest` to verify.
