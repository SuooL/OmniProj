import { expect, test } from "@playwright/test";

import { installMockTauri } from "./support/harness";

test.beforeEach(async ({ page }) => {
  await installMockTauri(page);
});

test("smoke: the dense Index renders the 12-project fixture", async ({ page }) => {
  await page.goto("/projects");
  await expect(page.locator(".op-row")).toHaveCount(12);
});

test("language switch updates the whole shell and persists across reloads", async ({ page }) => {
  await page.goto("/projects");
  await page.getByRole("button", { name: "Settings" }).click();
  const language = page.getByRole("combobox", { name: "Interface language" });
  await expect(language).toHaveValue("en");

  await language.selectOption("zh-CN");
  await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "界面语言" })).toHaveValue("zh-CN");

  await page.reload();
  await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "界面语言" })).toHaveValue("zh-CN");
  await page.getByRole("button", { name: "返回项目列表" }).click();
  await expect(page.getByRole("heading", { name: "项目", exact: true })).toBeVisible();
});

test("core loop: filter, open the project page, switch away, Undo, and return", async ({ page }) => {
  await page.goto("/projects");
  await page.getByLabel(/filter projects/i).fill("billing");
  const row = page.getByRole("link", { name: /^billing-worker/ });
  await expect(row).toBeVisible();
  await row.click();

  const overview = page.getByTestId("overview-page");
  await expect(overview).toBeVisible();
  await expect(overview.getByText("Idempotent retries")).toBeVisible();

  // Switching away releases the step back to the list; there is no separate replace form.
  await overview.getByRole("button", { name: "Switch away" }).click();
  await expect(overview.getByText("No step picked yet.")).toBeVisible();

  await expect(overview.getByTestId("undo-button")).toBeVisible();
  await overview.getByTestId("undo-button").click();
  // Undo is a real inverse: the prior step is restored.
  await expect(overview.getByText("Idempotent retries")).toBeVisible();

  await page.getByRole("button", { name: "Back to projects" }).click();
  await expect(page.getByTestId("projects-index")).toBeVisible();
});

test("a direct deep link renders the full page, not a Peek", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await expect(page.getByTestId("overview-page")).toBeVisible();
  await expect(page.getByTestId("overview-peek")).toHaveCount(0);
  await expect(page.getByTestId("overview-heading")).toHaveText("billing-worker");
});

test("browser history moves between the Index and the project page", async ({ page }) => {
  await page.goto("/projects");
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await expect(page.getByTestId("overview-page")).toBeVisible();

  await page.goBack();
  await expect(page.getByTestId("overview-page")).toHaveCount(0);
  await expect(page.getByTestId("projects-index")).toBeVisible();

  await page.goForward();
  await expect(page.getByTestId("overview-page")).toBeVisible();
});

test("Add Project registers a valid directory and opens the new setup project", async ({ page }) => {
  await page.goto("/projects");
  await page.evaluate(() => ((window as any).__mock.pick = "/valid/repo"));
  await page.getByRole("button", { name: "New project" }).click();

  const dialog = page.getByTestId("add-project-dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: /choose directory/i }).click();
  await expect(dialog.getByTestId("valid-preview")).toBeVisible();
  await dialog.getByRole("button", { name: "Register" }).click();

  await expect(page).toHaveURL(/\/projects\/new-proj\/overview$/);
  await expect(page.getByLabel("Objective")).toBeFocused();
});

test("relink recovers a moved source with explicit confirmation", async ({ page }) => {
  await page.goto("/projects/p01/overview");
  await page.evaluate(() => ((window as any).__mock.pick = "/valid/new"));
  const rec = page.getByTestId("source-recovery");
  await expect(rec).toBeVisible();
  await rec.getByRole("button", { name: /choose new location/i }).click();
  await rec.getByLabel("Confirm relink").check();
  await rec.getByRole("button", { name: "Relink source" }).click();
  // On success the source-recovery affordance disappears (source is available again).
  await expect(page.getByTestId("source-recovery")).toHaveCount(0);
});

test("a refresh with a partial source failure completes and announces the failure", async ({ page }) => {
  await page.goto("/projects");
  await page.evaluate(() => ((window as any).__mock.refreshFail = ["p01"]));
  await page.getByRole("button", { name: "Refresh" }).click();
  // Pull-refresh re-observes via refresh_projects; a source that failed is announced assertively,
  // not silently dropped, and the Index still renders every project.
  await expect(page.getByTestId("live-assertive")).toHaveText(/could not be refreshed/i);
  await expect(page.locator(".op-row")).toHaveCount(12);
});

test("a fully successful refresh announces completion politely", async ({ page }) => {
  await page.goto("/projects");
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByTestId("live-polite")).toHaveText(/projects refreshed/i);
});

test("a save failure surfaces the error and offers Retry", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.evaluate(() => ((window as any).__mock.failNext = "store_write_failed"));
  await page.getByRole("button", { name: "Complete" }).click();

  await expect(page.getByTestId("write-error")).toBeVisible();
  await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
  // The step is untouched, so there is no draft to preserve and nothing to copy.
  await expect(page.getByText("Idempotent retries")).toBeVisible();
});

test("completing the current step leaves no replacement", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByRole("button", { name: "Complete" }).click();
  await expect(page.getByText("No step picked yet.")).toBeVisible();
  await expect(page.getByText("Idempotent retries")).toHaveCount(0);
});

test("planning task creation is revisioned and appears without a page reload", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByText("Planning and tasks", { exact: true }).click();
  const board = page.getByTestId("task-board");
  await board.getByLabel("New task").fill("Validate retry behavior under failover");
  await board.getByRole("button", { name: "Add task" }).click();
  await expect(board.getByText("Validate retry behavior under failover")).toBeVisible();
});

test("Agent settings enable the explicit Advance and adopt loop", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const settings = page.getByTestId("agent-settings");
  await settings.getByRole("combobox", { name: /^Provider$/ }).selectOption("deepseek");
  await settings.getByRole("textbox", { name: /^Model$/ }).fill("deepseek-chat");
  await settings.getByLabel(/I agree to send task text/i).check();
  await settings.getByRole("button", { name: "Save Agent settings" }).click();
  await expect(settings.getByText("Agent settings saved.")).toBeVisible();
  await settings.getByRole("button", { name: "Test connection" }).click();
  await expect(settings.getByText("Agent connection is ready.")).toBeVisible();

  await page.getByRole("button", { name: "Back to projects" }).click();
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await page.getByText("Planning and tasks", { exact: true }).click();
  const board = page.getByTestId("task-board");
  await board.getByLabel("New task").fill("Fix the intermittent retry bug");
  await board.getByLabel("Not yet clear (?)").check();
  await board.getByRole("button", { name: "Add task" }).click();
  // Advance now lives in the row's edit panel, so the row is opened first.
  await board.getByRole("button", { name: /Fix the intermittent retry bug/ }).click();
  await board.getByRole("button", { name: "Ask Agent to break down" }).click();
  await expect(board.getByText("Write a regression test")).toBeVisible();
  await board.getByLabel("Write a regression test").check();
  await board.getByLabel("Implement the smallest fix").check();
  await board.getByRole("button", { name: "Adopt selected" }).click();
  await expect(board.getByText("Implement the smallest fix")).toBeVisible();
});

test("a task row stays read-only until opened, then autosaves due, tags, and status", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByText("Planning and tasks", { exact: true }).click();
  const board = page.getByTestId("task-board");
  await board.getByLabel("New task").fill("Drain the dead-letter queue");
  await board.getByRole("button", { name: "Add task" }).click();

  // Collapsed: one expandable control per task, no editing fields on screen.
  const row = board.getByRole("button", { name: /Drain the dead-letter queue/ });
  await expect(row).toHaveAttribute("aria-expanded", "false");
  await expect(board.getByLabel(/Tags: Drain the dead-letter queue/)).toHaveCount(0);
  await expect(board.getByRole("button", { name: "Save task" })).toHaveCount(0);

  // Opening reveals labelled fields; leaving the panel persists them.
  await row.click();
  await expect(row).toHaveAttribute("aria-expanded", "true");
  // A real date control and a token field, not typed strings.
  const due = board.getByLabel(/Expected completion date: Drain the dead-letter queue/);
  await expect(due).toHaveAttribute("type", "date");
  await due.fill("2026-08-01");
  const tags = board.getByLabel(/Tags: Drain the dead-letter queue/);
  await tags.fill("infra");
  await tags.press("Enter");
  await tags.fill("retry");
  await tags.press("Enter");
  // Closing the row persists it: autosave must not depend on focus leaving the panel,
  // because on macOS a click never moves keyboard focus to a button.
  await row.click();
  await expect(board.getByRole("button", { name: /Drain the dead-letter queue.*Overdue/ })).toBeVisible();
  await expect(board.getByRole("button", { name: /Drain the dead-letter queue.*infra.*retry/ })).toBeVisible();

  // Status is a single decisive control on the collapsed row and saves immediately.
  await board.getByLabel(/Task status: Drain the dead-letter queue/).selectOption("doing");
  await expect(board.getByLabel(/Task status: Drain the dead-letter queue/)).toHaveValue("doing");
});

test("the board keeps three aligned columns and marks empty ones", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByText("Planning and tasks", { exact: true }).click();
  const board = page.getByTestId("task-board");
  await board.getByLabel("New task").fill("Only open work");
  await board.getByRole("button", { name: "Add task" }).click();
  await board.getByRole("button", { name: "Board" }).click();

  const columns = board.getByTestId("task-board-columns");
  await expect(columns.locator(".op-board-col")).toHaveCount(3);
  // The two empty columns say so rather than collapsing into hollow bars.
  await expect(columns.getByText("None", { exact: true })).toHaveCount(2);
  const heights = await columns.locator(".op-board-col").evaluateAll((els) =>
    els.map((el) => Math.round(el.getBoundingClientRect().height)),
  );
  expect(new Set(heights).size).toBe(1);
});
