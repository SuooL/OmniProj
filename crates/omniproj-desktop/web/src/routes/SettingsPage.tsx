import { AgentSettings } from "../components/AgentSettings";
import { ReminderSettings } from "../components/ReminderSettings";
import { useI18n } from "../i18n/I18nProvider";

export function SettingsPage() {
  const { t } = useI18n();
  return (
    <main className="op-settings-page" aria-labelledby="settings-page-heading">
      <header className="op-page-heading">
        <p className="op-overview__eyebrow">{t("settingsPage.kicker")}</p>
        <h2 id="settings-page-heading">{t("settingsPage.title")}</h2>
        <p>{t("settingsPage.description")}</p>
      </header>
      <div className="op-settings-page__content">
        <ReminderSettings />
        <AgentSettings />
      </div>
    </main>
  );
}
