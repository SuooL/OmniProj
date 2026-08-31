// The fallback for any unknown route. It never dead-ends: the only action returns to the
// single primary destination, Projects.

import { Link } from "react-router-dom";

import { projectsPath } from "../domain/routes";
import { useI18n } from "../i18n/I18nProvider";

export function NotFoundPage() {
  const { t } = useI18n();
  return (
    <main className="op-empty-page" data-testid="not-found">
      <div className="op-empty-page__mark" aria-hidden="true">404</div>
      <p className="op-page-heading__eyebrow">{t("notFound.eyebrow")}</p>
      <h1>{t("notFound.title")}</h1>
      <p>{t("notFound.body")}</p>
      <Link className="op-button op-button--primary" to={projectsPath()}>{t("notFound.back")}</Link>
    </main>
  );
}
