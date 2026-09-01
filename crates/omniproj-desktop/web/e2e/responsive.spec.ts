import { expect, test, type Page } from "@playwright/test";

import { installMockTauri } from "./support/harness";

test.beforeEach(async ({ page }) => {
  await installMockTauri(page);
});

async function expectNoHorizontalScroll(page: Page) {
  const info = await page.evaluate(() => {
    const de = document.documentElement;
    const offenders: string[] = [];
    document.querySelectorAll<HTMLElement>("*").forEach((el) => {
      const r = el.getBoundingClientRect();
      if (r.right > de.clientWidth + 0.5) {
        offenders.push(`${el.tagName}.${(el.className || "").toString().slice(0, 40)}=${Math.round(r.right)}`);
      }
    });
    return { scrollWidth: de.scrollWidth, clientWidth: de.clientWidth, offenders: offenders.slice(0, 8) };
  });
  expect(
    info.scrollWidth,
    `h-overflow: scrollWidth=${info.scrollWidth} clientWidth=${info.clientWidth} offenders=${JSON.stringify(info.offenders)}`,
  ).toBeLessThanOrEqual(info.clientWidth);
}

test("1280x800: the focus-first project queue fits without permanent navigation chrome", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/projects");
  await expect(page.locator(".op-row")).toHaveCount(12);

  const visible = await page.evaluate(() =>
    Array.from(document.querySelectorAll(".op-row")).filter((r) => {
      const b = r.getBoundingClientRect();
      return b.top < window.innerHeight && b.bottom > 0;
    }).length,
  );
  expect(visible, `visible rows=${visible}`).toBeGreaterThanOrEqual(6);
  expect(visible).toBeLessThanOrEqual(10);
  await expect(page.locator(".app-shell__sidebar")).toHaveCount(0);
  await expect(page.locator(".app-shell__topbar")).toBeVisible();
  await expect(page.locator(".op-index__head")).toHaveCount(0);
  await expectNoHorizontalScroll(page);
});

test("1100: selecting a project replaces the queue and keeps its name in context", async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 800 });
  await page.goto("/projects");
  await expect(page.locator(".op-row__metadata").first()).toBeVisible();
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await expect(page.getByTestId("overview-page")).toBeVisible();
  await expect(page.locator(".app-shell__context-name")).toHaveText("billing-worker");
  await expectNoHorizontalScroll(page);
});

test("1099 and 800: the desktop list compresses without introducing a table or horizontal scroll", async ({ page }) => {
  for (const width of [1099, 800]) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto("/projects");
    await expect(page.locator(".op-index__head")).toHaveCount(0);
    await expect(page.locator(".app-shell__sidebar")).toHaveCount(0);
    await expectNoHorizontalScroll(page);
  }
});

test("799 and 640: compact chrome and project detail remain full-width", async ({ page }) => {
  for (const width of [799, 640]) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto("/projects");
    await expect(page.locator(".app-shell__sidebar")).toHaveCount(0);
    await expect(page.locator(".app-shell__topbar")).toBeVisible();
    await expectNoHorizontalScroll(page);

    await page.getByRole("link", { name: /^billing-worker/ }).click();
    await expect(page.getByTestId("overview-page")).toBeVisible();
    await expect(page.getByTestId("projects-index")).toHaveCount(0);
    await expectNoHorizontalScroll(page);
  }
});

test("200% text: no horizontal overflow and actions stay reachable", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  // Double every text token — a faithful text-only zoom (components consume these tokens).
  await page.addInitScript(() => {
    const style = document.createElement("style");
    style.textContent = `:root{--op-text-size-micro:22px;--op-text-size-necessary:24px;--op-text-size-body:26px;--op-text-size-emphasis:30px;--op-line-necessary:32px;--op-line-body:36px;}`;
    document.documentElement.appendChild(style);
  });
  await page.goto("/projects");
  await expectNoHorizontalScroll(page);

  // Rows grow with the larger text (min-height is a floor, not a fixed height) rather than
  // clipping it: content height never exceeds the row's own height.
  const clipped = await page.evaluate(() =>
    Array.from(document.querySelectorAll<HTMLElement>(".op-row__link")).some(
      (r) => r.scrollHeight > r.clientHeight + 1,
    ),
  );
  expect(clipped, "no row clips its enlarged text").toBe(false);

  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await expect(page.getByTestId("overview-page").getByRole("button", { name: "Replace" })).toBeVisible();
  await expectNoHorizontalScroll(page);
});
