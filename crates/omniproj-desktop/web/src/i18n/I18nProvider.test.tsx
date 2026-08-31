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
