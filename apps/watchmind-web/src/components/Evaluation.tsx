import { useEffect, useState } from "react";
import { api, type EvaluationReport, type EvaluationResult } from "../api";
import { Button, StatePanel } from "./Primitives";

const names = { random: "Hasard déterministe", anilist_global_score: "Note globale AniList", tag_overlap: "Tags en commun" };

function MetricCard({ result, engine = false }: { result: EvaluationResult; engine?: boolean }) {
  return <article className={engine ? "evaluation-engine" : undefined}>
    <p className="eyebrow">{engine ? "Moteur WatchMind" : names[result.name as keyof typeof names]}</p>
    {engine && <span className="evaluation-engine__label">Profil reconstruit sans la cible</span>}
    <strong className="metric-main">{Math.round(result.metrics.recall_at_10 * 100)}<small>%</small></strong>
    <span>retrouvés dans le top 10</span>
    <dl><div><dt>Rang médian</dt><dd>{result.metrics.median_rank.toFixed(1)}</dd></div><div><dt>Top 20</dt><dd>{Math.round(result.metrics.recall_at_20 * 100)}%</dd></div><div><dt>Rang réciproque</dt><dd>{result.metrics.mean_reciprocal_rank.toFixed(2)}</dd></div></dl>
  </article>;
}

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
    <header className="page-header evaluation-hero"><div><p className="eyebrow">Évaluation locale / {report.cases} cas</p><h1>Le moteur face aux raccourcis.</h1><p className="page-header__intro">Chaque favori est caché à son tour. Le profil est reconstruit sans lui, puis WatchMind doit le retrouver parmi des œuvres inédites.</p></div><span className="evaluation-threshold"><strong>≥ {report.configuration.relevant_rating_threshold}/10</strong><small>note considérée pertinente</small></span></header>
    <section className="evaluation-board" aria-labelledby="evaluation-title"><header className="decision-heading"><div><p className="eyebrow">Test à armes égales</p><h2 id="evaluation-title">Moteur et points de comparaison</h2></div><p>Les quatre méthodes classent les mêmes œuvres sur les mêmes {report.cases} cas.</p></header><div className="baseline-grid"><MetricCard result={report.engine} engine />{report.baselines.map((baseline) => <MetricCard key={baseline.name} result={baseline} />)}</div></section>
    <section className="pipeline-journey" aria-labelledby="pipeline-title">
      <header><p className="eyebrow">Pipeline réellement livré</p><h2 id="pipeline-title">Où disparaissent les bonnes recommandations ?</h2></header>
      <div className="pipeline-journey__track">
        <div><strong>{report.pipeline.cases}</strong><span>favoris masqués</span></div><i aria-hidden="true" />
        <div><strong>{report.pipeline.retrieved}</strong><span>passent le retrieval · {Math.round(report.pipeline.retrieval_recall * 100)}%</span></div><i aria-hidden="true" />
        <div><strong>{report.pipeline.listed}</strong><span>atteignent la liste · {Math.round(report.pipeline.list_recall * 100)}%</span></div>
      </div>
      <p>Le premier écart mesure la sélection des candidats ; le second, le classement et la diversification.</p>
    </section>
    <section className="reserve-lab" aria-labelledby="reserve-title">
      <header className="decision-heading"><div><p className="eyebrow">Réserve de popularité</p><h2 id="reserve-title">Le réglage est mesuré, pas supposé.</h2></div><p>Chaque variante rejoue les {report.cases} favoris sur le même manifeste de {report.catalog_manifest.work_ids.length} œuvres.</p></header>
      <div className="reserve-lab__grid">{report.popularity_reserve_sweep.map((result) => <article key={result.popularity_reserve} className={result.popularity_reserve === .25 ? "is-current" : undefined}><small>{Math.round(result.popularity_reserve * 100)}% populaire</small><strong>{Math.round(result.pipeline.list_recall * 100)}%</strong><span>atteignent la liste</span><em>{Math.round(result.pipeline.retrieval_recall * 100)}% passent le retrieval</em></article>)}</div>
      <footer><span>Pool AniList · {report.catalog_manifest.discovery_tags.join(" · ") || "popularité générale"}</span><span>{new Date(report.catalog_manifest.generated_at_unix * 1000).toLocaleString("fr-FR")}</span></footer>
    </section>
    <section className="evaluation-note"><p className="eyebrow">Décroissance temporelle</p><p>{report.temporal_backtest.available ? `Backtest actif sur ${report.temporal_backtest.cases} cas datés : Recall@10 ${Math.round((report.temporal_backtest.metrics?.recall_at_10 ?? 0) * 100)}%.` : "Les nouvelles notes sont désormais datées. Le backtest s’activera dès que l’historique daté sera suffisant ; aucune date artificielle n’est attribuée aux anciennes notes."}</p></section>
    <section className="evaluation-note"><p className="eyebrow">Comment lire ce panneau</p><p>Un top 10 élevé et un rang médian bas sont préférables. WatchMind n’est utile que s’il rivalise avec les raccourcis simples — popularité AniList et tags en commun — sans connaître la note masquée.</p></section>
  </>;
}
