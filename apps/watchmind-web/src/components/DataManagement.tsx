import { ChangeEvent, useRef, useState } from "react";
import { api } from "../api";
import { Button } from "./Primitives";

type Status = { tone: "success" | "error"; message: string } | null;

export function DataManagement() {
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState<"export" | "import" | "wipe" | null>(null);
  const [status, setStatus] = useState<Status>(null);

  async function exportDatabase() {
    setBusy("export"); setStatus(null);
    try {
      const blob = await api.exportDatabase();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      const date = new Date().toISOString().slice(0, 10);
      link.href = url; link.download = `watchmind-backup-${date}.json`; link.click();
      URL.revokeObjectURL(url);
      setStatus({ tone: "success", message: "Sauvegarde téléchargée. Conservez ce fichier pour restaurer votre profil." });
    } catch (reason) {
      setStatus({ tone: "error", message: reason instanceof Error ? reason.message : "Export impossible." });
    } finally { setBusy(null); }
  }

  async function importDatabase(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || !window.confirm("Remplacer toutes les données actuelles par cette sauvegarde ?")) return;
    setBusy("import"); setStatus(null);
    try {
      await api.importDatabase(await file.text());
      setStatus({ tone: "success", message: "Sauvegarde restaurée. Les pages utiliseront désormais les données importées." });
    } catch (reason) {
      setStatus({ tone: "error", message: reason instanceof Error ? reason.message : "Import impossible." });
    } finally { setBusy(null); }
  }

  async function wipeDatabase() {
    if (window.prompt("Cette action est irréversible. Tapez EFFACER pour vider WatchMind.") !== "EFFACER") return;
    setBusy("wipe"); setStatus(null);
    try {
      await api.clearDatabase();
      setStatus({ tone: "success", message: "Base effacée. WatchMind est revenu à un profil vide." });
    } catch (reason) {
      setStatus({ tone: "error", message: reason instanceof Error ? reason.message : "Suppression impossible." });
    } finally { setBusy(null); }
  }

  return <>
    <header className="page-header data-hero"><div><p className="eyebrow">Archives locales</p><h1>Vos données vous appartiennent.</h1><p className="page-header__intro">Emportez tout le profil, restaurez une sauvegarde ou repartez de zéro. Aucun fichier n’est envoyé ailleurs.</p></div><span className="data-stamp"><strong>JSON</strong><small>format versionné complet</small></span></header>
    <section className="data-actions" aria-labelledby="data-actions-title">
      <header className="decision-heading"><div><p className="eyebrow">Copie complète</p><h2 id="data-actions-title">Sauvegarder et restaurer</h2></div><p>La sauvegarde contient la bibliothèque, les notes, les événements, les préférences et les versions du profil.</p></header>
      <div className="data-action-grid">
        <article><span className="data-action__mark" aria-hidden="true">↓</span><div><p className="eyebrow">Sortie</p><h3>Exporter la base</h3><p>Télécharge un fichier que WatchMind pourra relire à l’identique.</p><Button onClick={() => void exportDatabase()} disabled={busy !== null}>{busy === "export" ? "Préparation…" : "Exporter"}</Button></div></article>
        <article><span className="data-action__mark" aria-hidden="true">↑</span><div><p className="eyebrow">Entrée</p><h3>Importer une sauvegarde</h3><p>Vérifie le fichier puis remplace les données dans une seule transaction.</p><input ref={input} className="visually-hidden" type="file" accept="application/json,.json" onChange={(event) => void importDatabase(event)} /><Button tone="quiet" onClick={() => input.current?.click()} disabled={busy !== null}>{busy === "import" ? "Restauration…" : "Choisir un fichier"}</Button></div></article>
      </div>
    </section>
    <section className="data-danger" aria-labelledby="data-danger-title"><div><p className="eyebrow">Zone irréversible</p><h2 id="data-danger-title">Vider WatchMind</h2><p>Supprime toutes les œuvres, notes, préférences et versions enregistrées. Exportez d’abord si vous souhaitez pouvoir revenir en arrière.</p></div><Button tone="danger" onClick={() => void wipeDatabase()} disabled={busy !== null}>{busy === "wipe" ? "Suppression…" : "Effacer toute la base"}</Button></section>
    {status && <p className={`data-status data-status--${status.tone}`} role="status">{status.message}</p>}
  </>;
}
