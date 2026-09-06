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
  { label: "row name", selector: ".op-row__name", min: 4.5 },
  { label: "row metadata", selector: ".op-row__metadata", min: 4.5 },
  { label: "row steps left", selector: ".op-row__steps", min: 4.5 },
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

async function expectNoSeriousAxe(page: Page, label: string) {
  const results = await new AxeBuilder({ page }).analyze();
  const serious = results.violations.filter(
    (v) => v.impact === "critical" || v.impact === "serious",
  );
  expect(serious, `${label}: ${JSON.stringify(serious.map((v) => v.id))}`).toEqual([]);
}

test("axe: the full Overview page has no critical/serious violations", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await expect(page.getByTestId("overview-page")).toBeVisible();
  await expectNoSeriousAxe(page, "overview-page");
});

test("axe: project navigation and the Add Project dialog have no critical/serious violations", async ({ page }) => {
  await page.goto("/projects");
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  await expect(page.getByTestId("overview-page")).toBeVisible();
  await expectNoSeriousAxe(page, "project-page");

  await page.getByRole("button", { name: "New project" }).click();
  await expect(page.getByTestId("add-project-dialog")).toBeVisible();
  await expectNoSeriousAxe(page, "add-project-dialog");
});

test("Project page text (git facts, source path, step markers) meets >=4.5:1 contrast", async ({ page }) => {
  await page.goto("/projects/p04/overview");
  await expect(page.getByTestId("task-outline")).toBeVisible();
  await page.getByText("Commits and branch graph", { exact: true }).click();
  for (const selector of [".op-project__facts", ".op-source-path", ".op-outline__marker", ".op-heatmap__caption"]) {
    if ((await page.locator(selector).count()) === 0) continue;
    const { fg, bg } = await readPair(page, selector);
    const ratio = contrastRatio(fg, bg);
    test.info().annotations.push({ type: "contrast", description: `overview ${selector}: ${fg} on ${bg} = ${ratio.toFixed(2)}:1` });
    expect(ratio, `${selector} (${fg} on ${bg})`).toBeGreaterThanOrEqual(4.5);
  }
});

test("prefers-contrast: more keeps the lifecycle tag readable", async ({ page }) => {
  await page.emulateMedia({ contrast: "more" });
  await page.goto("/projects");
  await expect(page.locator(".op-tag").first()).toBeVisible();
  const { fg, bg } = await readPair(page, ".op-tag");
  const ratio = contrastRatio(fg, bg);
  test.info().annotations.push({ type: "contrast", description: `high-contrast tag: ${fg} on ${bg} = ${ratio.toFixed(2)}:1` });
  expect(ratio).toBeGreaterThanOrEqual(4.5);
});

test("color-vision deficiency: signals stay legible because colour is redundant with text", async ({ page }) => {
  await page.goto("/projects");
  const cdp = await page.context().newCDPSession(page);
  for (const type of ["deuteranopia", "protanopia", "tritanopia", "achromatopsia"] as const) {
    await cdp.send("Emulation.setEmulatedVisionDeficiency", { type });
    // Every signal on the row is words, not colour: the lifecycle tag and the step count.
    await expect(page.locator(".op-tag", { hasText: "Waiting" })).toBeVisible();
    await expect(page.getByText("No steps yet").first()).toBeVisible();
  }
  await cdp.send("Emulation.setEmulatedVisionDeficiency", { type: "none" });
});

test("control boundaries meet >=3:1 (filter input border vs surface)", async ({ page }) => {
  await page.goto("/projects");
  await expect(page.locator(".op-index__search input")).toBeVisible();
  const { border, bg } = await page.evaluate(() => {
    const el = document.querySelector(".op-index__search input")!;
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
  await expect(page.locator(".op-tag", { hasText: "Waiting" })).toBeVisible();
  await expect(page.locator(".op-tag", { hasText: "Setup" })).toBeVisible();
  await expect(page.getByText("No steps yet").first()).toBeVisible();
});

test("forced-colors and reduced-motion keep labels and boundaries", async ({ page }) => {
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await page.goto("/projects");
  await expect(page.getByRole("list", { name: "Projects" })).toBeVisible();
  await expect(page.locator(".op-tag", { hasText: "Waiting" })).toBeVisible();

  // Reduced motion genuinely collapses transitions (a row normally animates on hover).
  const durationMs = await page.evaluate(() => {
    const link = document.querySelector(".op-row__link");
    if (!link) return null;
    return getComputedStyle(link)
      .transitionDuration.split(",")
      .map((d) => (d.trim().endsWith("ms") ? parseFloat(d) : parseFloat(d) * 1000))
      .reduce((max, v) => Math.max(max, v), 0);
  });
  expect(durationMs, "transitions collapse under reduced motion").not.toBeNull();
  expect(durationMs as number).toBeLessThanOrEqual(1);

  // Open a project page: its heading and primary action remain reachable.
  await page.getByRole("link", { name: /^billing-worker/ }).click();
  const outline = page.getByTestId("overview-page").getByTestId("task-outline");
  await expect(outline.getByRole("button", { name: "Idempotent retries" })).toBeVisible();
  await expect(outline.getByRole("button", { name: "Add step" })).toBeVisible();
});

// --- Interaction audit gates ------------------------------------------------
// These encode the rules the R2 audit sweep used, so the defects it found cannot
// silently return: a control must be hittable, must say why it is disabled, must not
// be labelled by its placeholder alone, and a label/control pair must stay within a
// readable measure instead of stretching across a wide pane.

const AUDIT = `(() => {
  const out = [];
  const vis = (el) => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };
  const name = (el) => (el.getAttribute('aria-label') || el.getAttribute('title') || (el.labels && el.labels[0] && el.labels[0].textContent) || el.textContent || '').trim();
  document.querySelectorAll('button, a[href], select, [role=button], [role=tab]').forEach((el) => {
    if (!vis(el)) return;
    const r = el.getBoundingClientRect();
    if (r.height < 26 || r.width < 26) out.push('hit-target: ' + Math.round(r.width) + 'x' + Math.round(r.height) + ' "' + name(el).slice(0, 30) + '"');
  });
  document.querySelectorAll('button:disabled, input:disabled, select:disabled').forEach((el) => {
    if (vis(el) && !el.getAttribute('title') && !el.getAttribute('aria-describedby')) out.push('disabled-unexplained: "' + name(el).slice(0, 30) + '"');
  });
  document.querySelectorAll('input[placeholder], textarea[placeholder]').forEach((el) => {
    if (!vis(el)) return;
    if (!(el.getAttribute('aria-label') || el.getAttribute('aria-labelledby') || (el.labels && el.labels.length))) out.push('placeholder-as-label: "' + el.getAttribute('placeholder') + '"');
  });
  // Only SIDE-BY-SIDE label/control pairs have a travel problem; a stacked field (label
  // above its control) is fine at any width, so the rule measures the horizontal offset
  // rather than the row width.
  document.querySelectorAll('label').forEach((lab) => {
    if (!vis(lab)) return;
    const ctl = lab.querySelector('select, input, textarea');
    if (!ctl || !vis(ctl)) return;
    const l = lab.getBoundingClientRect();
    const c = ctl.getBoundingClientRect();
    const sideBySide = c.left - l.left > 24;
    if (sideBySide && c.left - l.left > 420) out.push('label-control-travel: ' + Math.round(c.left - l.left) + 'px "' + lab.textContent.trim().slice(0, 20) + '"');
  });
  document.querySelectorAll('*').forEach((el) => {
    if (!el.childNodes.length || !vis(el)) return;
    if (!Array.from(el.childNodes).some((n) => n.nodeType === 3 && n.textContent.trim())) return;
    const size = parseFloat(getComputedStyle(el).fontSize);
    if (size && size < 11) out.push('text-too-small: ' + size + 'px "' + el.textContent.trim().slice(0, 20) + '"');
  });
  return out;
})()`;

for (const [label, url] of [
  ["projects index", "/projects"],
  ["project overview", "/projects/p04/overview"],
  ["settings", "/settings"],
]) {
  test(`interaction audit: ${label} has hittable, explained, measured controls`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(url);
    if (url.includes("overview")) {
      // Include the outline and one open step's detail panel in the sweep.
      const outline = page.getByTestId("task-outline");
      await outline.getByLabel("Add step").fill("Audited step");
      await outline.getByRole("button", { name: "Add step" }).click();
      await outline.getByTestId("outline-row").last().getByRole("button", { name: "Details" }).click();
    }
    const findings = await page.evaluate(AUDIT);
    expect(findings, `audit findings on ${label}:\n${findings.join("\n")}`).toEqual([]);
  });
}
