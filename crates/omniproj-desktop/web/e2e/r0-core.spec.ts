import { expect, test } from "@playwright/test";

import { installMockTauri } from "./support/harness";

test.beforeEach(async ({ page }) => {
  await installMockTauri(page);
});

test("smoke: the dense Index renders the 12-project fixture", async ({ page }) => {
  await page.goto("/projects");
  await expect(page.getByRole("list", { name: "Projects" }).getByRole("listitem")).toHaveCount(12);
});

test("language switch updates the whole shell and persists across reloads", async ({ page }) => {
  await page.goto("/projects");
  const language = page.getByRole("combobox", { name: "Interface language" });
  await expect(language).toHaveValue("en");

  await language.selectOption("zh-CN");
  await expect(page.getByRole("heading", { name: "项目" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "界面语言" })).toHaveValue("zh-CN");

  await page.reload();
  await expect(page.getByRole("heading", { name: "项目" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "界面语言" })).toHaveValue("zh-CN");
});

test("core loop: filter, open the project page, replace with explicit Save, Undo, and return", async ({ page }) => {
  await page.goto("/projects");
  await page.getByLabel(/filter projects/i).fill("billing");
  const row = page.getByRole("link", { name: /^billing-worker/ });
  await expect(row).toBeVisible();
  await row.click();

  const overview = page.getByTestId("overview-page");
  await expect(overview).toBeVisible();
  await expect(overview.getByText("Idempotent retries")).toBeVisible();

  await overview.getByRole("button", { name: "Replace" }).click();
  await overview.getByLabel("New commitment").fill("Exactly-once delivery");
  await overview.getByLabel("Replace reason").fill("scope narrowed");
  await overview.getByRole("button", { name: "Save replacement" }).click();

  await expect(overview.getByText("Exactly-once delivery")).toBeVisible();
  await expect(overview.getByTestId("undo-button")).toBeVisible();
  await overview.getByTestId("undo-button").click();
  // Undo is a real inverse: the prior commitment is restored.
  await expect(overview.getByText("Idempotent retries")).toBeVisible();
  await expect(overview.getByText("Exactly-once delivery")).toHaveCount(0);

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
  await page.getByRole("button", { name: "Add Project" }).click();

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
  await expect(page.getByRole("list", { name: "Projects" }).getByRole("listitem")).toHaveCount(12);
});

test("a fully successful refresh announces completion politely", async ({ page }) => {
  await page.goto("/projects");
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByTestId("live-polite")).toHaveText(/projects refreshed/i);
});

test("a save failure preserves the draft and offers Retry + Copy", async ({ page }) => {
  await page.goto("/projects/p03/overview"); // p03 has no commitment -> set form
  await page.evaluate(() => ((window as any).__mock.failNext = "store_write_failed"));
  await page.getByLabel("New commitment").fill("draft that must survive");
  await page.getByRole("button", { name: "Save commitment" }).click();

  await expect(page.getByTestId("write-error")).toBeVisible();
  await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy text" })).toBeVisible();
  await expect(page.getByLabel("New commitment")).toHaveValue("draft that must survive");
});

test("completing a commitment leaves no replacement", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByRole("button", { name: "Complete" }).click();
  await expect(page.getByTestId("set-form")).toBeVisible(); // now shows the empty set form
  await expect(page.getByText("Idempotent retries")).toHaveCount(0);
});

test("planning task creation is revisioned and appears without a page reload", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await page.getByRole("button", { name: "Plan" }).click();
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

  await page.getByRole("button", { name: "billing-worker" }).click();
  await page.getByRole("button", { name: "Plan" }).click();
  const board = page.getByTestId("task-board");
  await board.getByLabel("New task").fill("Fix the intermittent retry bug");
  await board.getByLabel("Not yet clear (?)").check();
  await board.getByRole("button", { name: "Add task" }).click();
  await board.getByRole("button", { name: "Ask Agent to break down" }).click();
  await expect(board.getByText("Write a regression test")).toBeVisible();
  await board.getByLabel("Write a regression test").check();
  await board.getByLabel("Implement the smallest fix").check();
  await board.getByRole("button", { name: "Adopt selected" }).click();
  await expect(board.getByText("Implement the smallest fix")).toBeVisible();
});
