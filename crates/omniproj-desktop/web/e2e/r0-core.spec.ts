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

test("core loop: filter, open the project, tick a step off, and return", async ({ page }) => {
  await page.goto("/projects");
  await page.getByLabel(/filter projects/i).fill("billing");
  const row = page.getByRole("link", { name: /^billing-worker/ });
  await expect(row).toBeVisible();
  await row.click();

  const overview = page.getByTestId("overview-page");
  await expect(overview).toBeVisible();

  // The steps are the page itself — no tab, no second click.
  const outline = overview.getByTestId("task-outline");
  await expect(outline.getByRole("button", { name: "Idempotent retries" })).toBeVisible();

  const check = outline.getByRole("checkbox", { name: /Idempotent retries/ });
  await expect(check).not.toBeChecked();
  await check.click();
  await expect(check).toBeChecked();

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

test("Add Project registers a valid directory and asks only for the first step", async ({ page }) => {
  await page.goto("/projects");
  await page.evaluate(() => ((window as any).__mock.pick = "/valid/repo"));
  await page.getByRole("button", { name: "New project" }).click();

  const dialog = page.getByTestId("add-project-dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: /choose directory/i }).click();
  await expect(dialog.getByTestId("valid-preview")).toBeVisible();
  await dialog.getByRole("button", { name: "Register" }).click();

  await expect(page).toHaveURL(/\/projects\/new-proj\/overview$/);
  await expect(page.getByLabel("First step")).toBeFocused();

  // One field, one button, and the project is usable.
  await page.getByLabel("First step").fill("Read the ingest pipeline");
  await page.getByRole("button", { name: "Start this project" }).click();
  await expect(page.getByTestId("task-outline").getByRole("button", { name: "Read the ingest pipeline" })).toBeVisible();
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

test("a failed step write says so and leaves the step untouched", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  const outline = page.getByTestId("task-outline");
  await expect(outline.getByRole("button", { name: "Idempotent retries" })).toBeVisible();

  await page.evaluate(() => ((window as any).__mock.failNext = "store_write_failed"));
  await outline.getByRole("checkbox", { name: /Idempotent retries/ }).click();

  await expect(page.getByTestId("outline-error")).toBeVisible();
  // The write failed, so the list is reloaded from the store rather than left optimistic.
  await expect(outline.getByRole("checkbox", { name: /Idempotent retries/ })).not.toBeChecked();
});

test("the whole outline is on the page; commits and settings are collapsed", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await expect(page.getByTestId("task-outline")).toBeVisible();
  await expect(page.getByTestId("commit-heatmap")).toBeVisible();

  // Reference material is closed until asked for.
  const commits = page.locator("details", { hasText: "Commits and branch graph" }).first();
  await expect(commits).not.toHaveAttribute("open", "");
  await commits.getByText("Commits and branch graph").click();
  await expect(page.getByText("/Users/dev/billing-worker")).toBeVisible();

  // The vocabulary the redesign removed is gone from the page.
  for (const gone of [/current commitment/i, /re-entry/i, /needs review/i, /decisions/i]) {
    await expect(page.getByText(gone)).toHaveCount(0);
  }
});

test("the outline nests, renumbers, and carries sub-steps with their parent", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  const outline = page.getByTestId("task-outline");

  await outline.getByLabel("Add step").fill("Extract reports");
  await outline.getByRole("button", { name: "Add step" }).click();
  await expect(outline.getByRole("button", { name: "Extract reports" })).toBeVisible();

  // Two top-level steps, numbered 1 and 2.
  const rows = outline.getByTestId("outline-row");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText("1.");
  await expect(rows.nth(1)).toContainText("2.");

  // Enter opens a new sibling; typing and Tab nests it under the step above.
  await outline.getByRole("button", { name: "Extract reports" }).click();
  await page.keyboard.press("Enter");
  await page.keyboard.type("OCR");
  await page.keyboard.press("Tab");
  // The row stays in edit mode after Tab (you keep typing), so close it to read the text back.
  await outline.getByRole("heading", { name: "Steps" }).click();

  const ocr = outline.getByTestId("outline-row").filter({ hasText: "OCR" });
  await expect(ocr).toHaveAttribute("data-depth", "1");
  // A sub-step takes no number of its own; the top level keeps counting.
  await expect(outline.getByTestId("outline-row")).toHaveCount(3);
  await expect(outline.getByTestId("outline-row").nth(1)).toContainText("2.");
});

test("Agent settings enable the explicit breakdown, adopted as sub-steps", async ({ page }) => {
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
  const outline = page.getByTestId("task-outline");

  // The breakdown lives in the step's own details panel.
  await outline.getByTestId("outline-row").first().getByRole("button", { name: "Details" }).click();
  await outline.getByRole("button", { name: "Break into sub-steps with AI" }).click();
  await expect(outline.getByText("Write a regression test")).toBeVisible();
  await outline.getByLabel("Write a regression test").check();
  await outline.getByLabel("Implement the smallest fix").check();
  await outline.getByRole("button", { name: "Adopt selected" }).click();

  // Adopted candidates land under the step they came from, not at the end of the list.
  const adopted = outline.getByTestId("outline-row").filter({ hasText: "Implement the smallest fix" });
  await expect(adopted).toHaveAttribute("data-depth", "1");
});

test("a step stays read-only until opened, then saves its due date and tags", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  const outline = page.getByTestId("task-outline");
  const row = outline.getByTestId("outline-row").first();

  // Collapsed: the text is a plain control, with no editing fields on screen.
  await expect(row.getByRole("button", { name: "Idempotent retries" })).toBeVisible();
  await expect(outline.getByLabel("Expected completion date")).toHaveCount(0);

  await row.getByRole("button", { name: "Details" }).click();
  const due = outline.getByLabel("Expected completion date");
  await expect(due).toHaveAttribute("type", "date");
  await due.fill("2026-08-01");
  const tags = outline.getByLabel("Tags");
  await tags.fill("infra");
  await tags.press("Enter");

  // The saved facts show on the collapsed row.
  await expect(row).toContainText("Overdue");
});

test("renaming a step in place persists it", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  const outline = page.getByTestId("task-outline");

  await outline.getByRole("button", { name: "Idempotent retries" }).click();
  const field = outline.getByLabel("Edit step text");
  await field.fill("Idempotent retries, with a dead-letter queue");
  await field.blur();

  await expect(outline.getByRole("button", { name: "Idempotent retries, with a dead-letter queue" })).toBeVisible();
});
