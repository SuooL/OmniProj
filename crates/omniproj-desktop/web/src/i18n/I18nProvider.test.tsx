import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import {
  I18nProvider,
  LOCALE_STORAGE_KEY,
  useI18n,
} from "./I18nProvider";

function Probe() {
  const { locale, setLocale, t } = useI18n();
  return (
    <div>
      <span data-testid="locale">{locale}</span>
      <span>{t("shell.projects")}</span>
      <button type="button" onClick={() => setLocale("en")}>English</button>
    </div>
  );
}

afterEach(() => window.localStorage.clear());

describe("Chinese-first locale", () => {
  it("defaults to Simplified Chinese when no preference exists", () => {
    window.localStorage.removeItem(LOCALE_STORAGE_KEY);
    render(<I18nProvider><Probe /></I18nProvider>);
    expect(screen.getByTestId("locale")).toHaveTextContent("zh-CN");
    expect(screen.getByText("项目")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("zh-CN");
  });

  it("switches to English, persists the choice, and restores it", async () => {
    window.localStorage.removeItem(LOCALE_STORAGE_KEY);
    const user = userEvent.setup();
    const first = render(<I18nProvider><Probe /></I18nProvider>);
    await user.click(screen.getByRole("button", { name: "English" }));
    expect(screen.getByText("Projects")).toBeInTheDocument();
    expect(window.localStorage.getItem(LOCALE_STORAGE_KEY)).toBe("en");
    expect(document.documentElement.lang).toBe("en");

    first.unmount();
    render(<I18nProvider><Probe /></I18nProvider>);
    expect(screen.getByTestId("locale")).toHaveTextContent("en");
    expect(screen.getByText("Projects")).toBeInTheDocument();
  });
});

describe("overdue review reason localization", () => {
  it("labels overdue_work in both locales", async () => {
    const { reviewReasonLabel } = await import("./I18nProvider");
    expect(reviewReasonLabel("overdue_work", "zh-CN")).toBe("任务逾期");
    expect(reviewReasonLabel("overdue_work", "en")).toBe("Overdue work");
  });

  it("localizes overdue evidence lines to Chinese and passes English through", async () => {
    const { localizeEvidence } = await import("./I18nProvider");
    expect(localizeEvidence("overdue items: 5", "zh-CN")).toBe("逾期任务：5 项");
    expect(localizeEvidence("due 2026-08-01 (9 days overdue): fix the parser", "zh-CN")).toBe(
      "预期 2026-08-01，已逾期 9 天：fix the parser",
    );
    expect(localizeEvidence("due 2026-08-10 (1 days overdue): 收尾", "zh-CN")).toBe(
      "预期 2026-08-10，已逾期 1 天：收尾",
    );
    expect(localizeEvidence("and 2 more overdue items", "zh-CN")).toBe("…另有 2 项逾期");
    expect(localizeEvidence("overdue items: 5", "en")).toBe("overdue items: 5");
  });
});
