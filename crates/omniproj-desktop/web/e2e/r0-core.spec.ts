import { expect, test } from "@playwright/test";

import { installMockTauri } from "./support/harness";

test.beforeEach(async ({ page }) => {
  await installMockTauri(page);
});

test("smoke: the dense Index renders the 12-project fixture", async ({ page }) => {
  await page.goto("/projects");
  await expect(page.getByRole("list", { name: "Projects" }).getByRole("listitem")).toHaveCount(12);
});

test("core loop: filter, open a Peek, replace with explicit Save, Undo, Escape restores the row", async ({ page }) => {
  await page.goto("/projects");
  await page.getByLabel(/filter projects/i).fill("billing");
  const row = page.getByRole("link", { name: /^billing-worker/ });
  await expect(row).toBeVisible();
  await row.click();

  const peek = page.getByTestId("overview-peek");
  await expect(peek).toBeVisible();
  await expect(peek.getByText("Idempotent retries")).toBeVisible();

  await peek.getByRole("button", { name: "Replace" }).click();
  await peek.getByLabel("New commitment").fill("Exactly-once delivery");
  await peek.getByLabel("Replace reason").fill("scope narrowed");
  await peek.getByRole("button", { name: "Save replacement" }).click();

  await expect(peek.getByText("Exactly-once delivery")).toBeVisible();
  await expect(peek.getByTestId("undo-button")).toBeVisible();
  await peek.getByTestId("undo-button").click();
  // Undo is a real inverse: the prior commitment is restored.
  await expect(peek.getByText("Idempotent retries")).toBeVisible();
  await expect(peek.getByText("Exactly-once delivery")).toHaveCount(0);

  await page.keyboard.press("Escape");
  await expect(page.getByTestId("overview-peek")).toHaveCount(0);
  await expect(row).toBeFocused();
});

test("a direct deep link renders the full page, not a Peek", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await expect(page.getByTestId("overview-page")).toBeVisible();
  await expect(page.getByTestId("overview-peek")).toHaveCount(0);
  await expect(page.getByTestId("overview-heading")).toHaveText("billing-worker");
});

test("Back and Forward move between the Index and the Peek", async ({ page }) => {
  await page.goto("/projects");
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await expect(page.getByTestId("overview-peek")).toBeVisible();

  await page.goBack();
  await expect(page.getByTestId("overview-peek")).toHaveCount(0);
  await expect(page.getByTestId("projects-index")).toBeVisible();

  await page.goForward();
  await expect(page.getByTestId("overview-peek")).toBeVisible();
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
