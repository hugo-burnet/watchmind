import { useEffect, useState } from "react";
import { api, type Contribution, type ProfileSnapshot, type Recommendation } from "../api";
import { Button, StatePanel } from "./Primitives";

function confidenceLabel(profile: ProfileSnapshot | null) {
  const value = profile?.profile.confidence ?? 0;
  if (value >= 0.66) return "Confiance solide";
  if (value >= 0.3) return "Confiance en construction";
  return "Peu de recul";
}

function reasonLabel(reason: Contribution) {
  if (reason.source.kind === "anilist_prior") return "Bien reçu au-delà de votre profil";
  if (reason.source.kind === "pole_similarity") return reason.detail.replace("Proximité", "Proche");
  if (reason.source.kind === "tag_affinity") return reason.detail.replace("Affinité apprise pour le tag ", "Vous aimez souvent : ");
  return reason.detail;
}

export function ForYou() {
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [profile, setProfile] = useState<ProfileSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  async function load() {
    setLoading(true); setError("");
    try {
      const [next, current] = await Promise.all([api.recommendations(), api.profile()]);
      setRecommendations(next.recommendations); setProfile(current);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Recommandations indisponibles."); }
    finally { setLoading(false); }
  }
  useEffect(() => { void load(); }, []);

  return <>
    <header className="page-header decision-hero">
      <div><p className="eyebrow">Pour vous / maintenant</p><h1>Votre prochaine histoire commence ici.</h1><p className="page-header__intro">Une sélection courte, avec les raisons de chaque trajet et ce qui pourrait moins vous convenir.</p></div>
      <span className="confidence-stamp"><i aria-hidden="true" />{confidenceLabel(profile)}<small>{profile?.profile.history_size ?? 0} œuvre{profile?.profile.history_size === 1 ? "" : "s"} observée{profile?.profile.history_size === 1 ? "" : "s"}</small></span>
    </header>
    {loading ? <div className="recommendation-state"><StatePanel eyebrow="Sélection" title="Les trajets se dessinent." busy>Lecture du dernier profil enregistré.</StatePanel></div>
    : error ? <div className="recommendation-state"><StatePanel eyebrow="Sélection indisponible" title="Le profil ne peut pas être lu." action={<Button tone="quiet" onClick={() => void load()}>Réessayer</Button>}>{error}</StatePanel></div>
    : recommendations.length === 0 ? <div className="recommendation-state"><StatePanel eyebrow="Aucun candidat" title="Ajoutez une œuvre à découvrir.">La bibliothèque contient vos repères, mais aucune œuvre non notée à comparer.</StatePanel></div>
    : <RecommendationBoard recommendations={recommendations} />}
  </>;
}

function RecommendationBoard({ recommendations }: { recommendations: Recommendation[] }) {
  const safe = recommendations.slice(0, 3);
  const bets = recommendations.slice(3, 5);
  return <div className="decision-board">
    <section aria-labelledby="safe-title"><header className="decision-heading"><div><p className="eyebrow">Trajets les plus lisibles</p><h2 id="safe-title">Choix sûrs</h2></div><p>Les correspondances les mieux étayées par votre profil actuel.</p></header>
      <div className="recommendation-list">{safe.map((item, index) => <RecommendationCard key={item.work_id} recommendation={item} rank={index + 1} />)}</div></section>
    {bets.length > 0 && <section className="bet-section" aria-labelledby="bets-title"><header className="decision-heading"><div><p className="eyebrow">Élargir la carte</p><h2 id="bets-title">Deux paris</h2></div><p>Un peu plus loin de vos repères, assez proches pour tenter le voyage.</p></header><div className="bet-grid">{bets.map((item, index) => <RecommendationCard key={item.work_id} recommendation={item} rank={index + 1} bet />)}</div></section>}
  </div>;
}

function RecommendationCard({ recommendation, rank, bet = false }: { recommendation: Recommendation; rank: number; bet?: boolean }) {
  const [feedback, setFeedback] = useState<boolean | null>(null);
  const reasons = recommendation.explanation.reasons.slice(0, 3);
  const risk = recommendation.explanation.risks[0];
  const pole = recommendation.score.contributions.find((item) => item.source.kind === "pole_similarity")?.detail;
  async function send(value: boolean) { await api.feedback(recommendation.work_id, value); setFeedback(value); }
  return <article className={bet ? "recommendation-card is-bet" : "recommendation-card"}>
    <div className="recommendation-route" aria-hidden="true"><span>{String(rank).padStart(2, "0")}</span><i /><b /></div>
    <div className="recommendation-main"><p className="eyebrow">{bet ? "Pari personnel" : pole ?? "Selon vos affinités"}</p><h3>{recommendation.title}</h3>
      <ol className="reason-list">{reasons.map((reason, index) => <li key={`${reason.detail}-${index}`}><span>{index + 1}</span>{reasonLabel(reason)}</li>)}</ol>
      <p className={risk ? "risk-line" : "risk-line is-clear"}><strong>{risk ? "À savoir" : "Aucun risque marqué"}</strong>{risk ? risk.detail : "Le moteur n’a pas relevé de signal négatif."}</p>
    </div>
    <div className="feedback"><span>{feedback === null ? "Cette piste vous aide ?" : "Retour enregistré"}</span><button onClick={() => void send(true)} aria-pressed={feedback === true} aria-label="Cette recommandation est utile">Oui</button><button onClick={() => void send(false)} aria-pressed={feedback === false} aria-label="Cette recommandation n’est pas utile">Non</button></div>
  </article>;
}
