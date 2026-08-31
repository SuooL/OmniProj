<!-- PRs target `dev` (trimmed Git Flow) — never `main`. See CONTRIBUTING.md. -->

## Summary

<!-- What does this change and why? -->

## Checklist

- [ ] Targets the `dev` branch (not `main`)
- [ ] Ran `./scripts/pre-pr-check.sh` from the repository root
- [ ] Ran `git diff --check` and verified the commit contains only intended files
- [ ] `cargo test --workspace` passes
- [ ] `cargo fmt --all --check` is clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] Added/updated tests for behavior changes
- [ ] Updated the relevant `docs/` spec (charter/requirements/desktop-design) and/or `README.md` if behavior changed
