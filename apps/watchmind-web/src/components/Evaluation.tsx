import { useEffect, useState } from "react";
import { api, type EvaluationReport } from "../api";
import { Button, StatePanel } from "./Primitives";

const names = { random: "Hasard déterministe", anilist_global_score: "Note globale AniList", tag_overlap: "Tags en commun" };

export function Evaluation() {
  const [report, setReport] = useState<EvaluationReport | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  async function load() {
    setLoading(true); setError("");
    try { setReport(await api.evaluation()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Évaluation indisponible."); }
    finally { setLoading(false); }
  }
  useEffect(() => { void load(); }, []);
  if (loading) return <div className="profile-state"><StatePanel eyebrow="Évaluation" title="Les repères sont masqués à tour de rôle." busy>Calcul des baselines sur votre historique local.</StatePanel></div>;
  if (error || !report) return <div className="profile-state"><StatePanel eyebrow="Historique insuffisant" title="Il faut au moins une œuvre notée 8 ou plus." action={<Button tone="quiet" onClick={() => void load()}>Recalculer</Button>}>{error || "Ajoutez une note forte pour créer un cas mesurable."}</StatePanel></div>;
  return <>
    <header className="page-header evaluation-hero"><div><p className="eyebrow">Évaluation locale / {report.cases} cas</p><h1>Mesurer sans se raconter d’histoire.</h1><p className="page-header__intro">Chaque favori est caché à son tour. WatchMind mesure alors si des méthodes simples savent le retrouver.</p></div><span className="evaluation-threshold"><strong>≥ {report.configuration.relevant_rating_threshold}/10</strong><small>note considérée pertinente</small></span></header>
    <section className="evaluation-board" aria-labelledby="evaluation-title"><header className="decision-heading"><div><p className="eyebrow">Baselines de contrôle</p><h2 id="evaluation-title">Points de comparaison</h2></div><p>Ces résultats décrivent votre jeu de données actuel, pas une promesse générale de précision.</p></header><div className="baseline-grid">{report.baselines.map((baseline) => <article key={baseline.name}><p className="eyebrow">{names[baseline.name]}</p><strong className="metric-main">{Math.round(baseline.metrics.recall_at_10 * 100)}<small>%</small></strong><span>retrouvés dans le top 10</span><dl><div><dt>Rang médian</dt><dd>{baseline.metrics.median_rank.toFixed(1)}</dd></div><div><dt>Top 20</dt><dd>{Math.round(baseline.metrics.recall_at_20 * 100)}%</dd></div><div><dt>Rang réciproque</dt><dd>{baseline.metrics.mean_reciprocal_rank.toFixed(2)}</dd></div></dl></article>)}</div></section>
    <section className="evaluation-note"><p className="eyebrow">Comment lire ce panneau</p><p>Un rappel top 10 élevé et un rang médian bas sont préférables. Le hasard vérifie le plancher ; AniList et les tags indiquent ce que des signaux simples obtiennent sans le moteur personnel.</p></section>
  </>;
}
