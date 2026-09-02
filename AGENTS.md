# Repository contribution gates

## Mandatory pre-commit / pre-PR verification

任何会进入 commit 或 PR 的代码改动，都必须在本地完成并记录以下完整检查；不得只运行受影响包的局部测试，也不得在错误工作目录下执行命令：

```sh
./scripts/pre-pr-check.sh
```

该脚本覆盖仓库 CI 的 Rust 格式、lint、workspace build/test，以及前端安装、build、unit test、Playwright E2E。脚本任一步失败时，禁止创建或更新 PR；应先修复并从头重跑。

提交前还必须确认：

- `git diff --check` 无输出；
- `git status --short` 只包含本次任务的预期改动；
- commit 后重新检查 `git show --check --stat HEAD`；
- push 后执行 `gh pr checks <number>`，在检查完成前不得宣称 CI 通过；
- 若 CI 失败，先读取失败 job 日志并修复，再重新验证和推送。

验证命令必须从仓库根目录启动；前端命令由脚本切换到 `crates/omniproj-desktop/web` 执行，避免因 cwd 错误漏检。
