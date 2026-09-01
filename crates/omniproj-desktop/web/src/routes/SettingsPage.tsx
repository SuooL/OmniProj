import { AgentSettings } from "../components/AgentSettings";
import { ReminderSettings } from "../components/ReminderSettings";
import { useI18n } from "../i18n/I18nProvider";

export function SettingsPage() {
  const { locale, setLocale, t } = useI18n();
  return (
    <main className="op-settings-page" aria-labelledby="settings-page-heading">
      <header className="op-page-heading">
        <p className="op-overview__eyebrow">{t("settingsPage.kicker")}</p>
        <h2 id="settings-page-heading">{t("settingsPage.title")}</h2>
        <p>{t("settingsPage.description")}</p>
      </header>
      <div className="op-settings-page__content">
        <section className="op-section op-settings-language" aria-labelledby="settings-language-heading">
          <div className="op-section__header"><h3 id="settings-language-heading">{t("language.label")}</h3></div>
          <label className="op-settings-language__control">
            <span>{t("language.label")}</span>
            <select aria-label={t("language.label")} value={locale} onChange={(event) => setLocale(event.target.value as "zh-CN" | "en")}>
              <option value="zh-CN">{t("language.zh")}</option>
              <option value="en">{t("language.en")}</option>
            </select>
          </label>
        </section>
        <ReminderSettings />
        <AgentSettings />
      </div>
    </main>
  );
}
