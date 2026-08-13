import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

import { installMockTauri } from "./support/harness";

test.beforeEach(async ({ page }) => {
  await installMockTauri(page);
});

// --- WCAG contrast helpers (Node side) -------------------------------------
function parseRgb(value: string): [number, number, number] {
  const m = value.match(/\d+(\.\d+)?/g);
  if (!m) throw new Error(`unparseable color: ${value}`);
  return [Number(m[0]), Number(m[1]), Number(m[2])];
}
function luminance([r, g, b]: [number, number, number]): number {
  const a = [r, g, b].map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2];
}
function contrastRatio(fg: string, bg: string): number {
  const [l1, l2] = [luminance(parseRgb(fg)), luminance(parseRgb(bg))];
  const [hi, lo] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (hi + 0.05) / (lo + 0.05);
}

/** Read {color, backgroundColor} of one element, falling back to the body bg when transparent. */
async function readPair(page: Page, selector: string): Promise<{ fg: string; bg: string }> {
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    if (!el) throw new Error(`missing ${sel}`);
    const cs = getComputedStyle(el);
    let bg = cs.backgroundColor;
    if (bg === "rgba(0, 0, 0, 0)" || bg === "transparent") {
      bg = getComputedStyle(document.body).backgroundColor;
    }
    return { fg: cs.color, bg };
  }, selector);
}

const TEXT_PAIRS = [
  { label: "ProjectStateTag", selector: ".op-tag", min: 4.5 },
  { label: "ReviewSignalBadge", selector: ".op-badge", min: 4.5 },
  { label: "row name", selector: ".op-row__name", min: 4.5 },
  { label: "observed note", selector: ".op-observed-note", min: 4.5 },
];

for (const scheme of ["light", "dark"] as const) {
  test(`axe finds no critical/serious violations (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/projects");
    await expect(page.getByRole("list", { name: "Projects" })).toBeVisible();
    const results = await new AxeBuilder({ page }).analyze();
    const serious = results.violations.filter(
      (v) => v.impact === "critical" || v.impact === "serious",
    );
    expect(serious, JSON.stringify(serious.map((v) => v.id), null, 2)).toEqual([]);
  });

  test(`semantic text meets >=4.5:1 contrast (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/projects");
    await expect(page.locator(".op-tag").first()).toBeVisible();
    for (const pair of TEXT_PAIRS) {
      const { fg, bg } = await readPair(page, pair.selector);
      const ratio = contrastRatio(fg, bg);
      test.info().annotations.push({
        type: "contrast",
        description: `${scheme} ${pair.label}: ${fg} on ${bg} = ${ratio.toFixed(2)}:1`,
      });
      expect(ratio, `${scheme} ${pair.label} (${fg} on ${bg})`).toBeGreaterThanOrEqual(pair.min);
    }
  });
}

test("control boundaries meet >=3:1 (filter input border vs surface)", async ({ page }) => {
  await page.goto("/projects");
  const { border, bg } = await page.evaluate(() => {
    const el = document.querySelector("input[type='search']")!;
    const cs = getComputedStyle(el);
    return { border: cs.borderTopColor, bg: getComputedStyle(document.body).backgroundColor };
  });
  const ratio = contrastRatio(border, bg);
  test.info().annotations.push({ type: "contrast", description: `control border: ${border} on ${bg} = ${ratio.toFixed(2)}:1` });
  expect(ratio).toBeGreaterThanOrEqual(3);
});

test("non-color semantics survive grayscale: badge text stays readable", async ({ page }) => {
  await page.addInitScript(() => {
    const s = document.createElement("style");
    s.textContent = "html{filter:grayscale(1)!important;}";
    document.documentElement.appendChild(s);
  });
  await page.goto("/projects");
  // Colour is stripped, but every signal is redundant with visible text.
  await expect(page.getByText("Source unavailable").first()).toBeVisible();
  await expect(page.getByText("Waiting").first()).toBeVisible();
  await expect(page.getByText("Complete setup").first()).toBeVisible();
});

test("forced-colors and reduced-motion keep labels and boundaries", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await page.goto("/projects");
  await expect(page.getByRole("list", { name: "Projects" })).toBeVisible();
  await expect(page.getByText("Source unavailable").first()).toBeVisible();
  // Open a Peek: its heading and primary action remain reachable.
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await expect(page.getByTestId("overview-peek").getByRole("button", { name: "Replace" })).toBeVisible();
});
