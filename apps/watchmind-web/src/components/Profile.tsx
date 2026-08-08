import { useEffect, useMemo, useState } from "react";
import { api, type CompleteWork, type ProfileSnapshot, type Recommendation } from "../api";
import { Button, StatePanel } from "./Primitives";

const axisLabels: Record<string, string> = { story: "Récit", characters: "Personnages", world_building: "Univers", visual_direction: "Mise en scène", sound_and_music: "Son & musique" };

export function Profile() {
  const [versions, setVersions] = useState<ProfileSnapshot[]>([]);
  const [works, setWorks] = useState<CompleteWork[]>([]);
  const [selectedVersion, setSelectedVersion] = useState<number | null>(null);
  const [history, setHistory] = useState<Recommendation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  async function load() {
    setLoading(true); setError("");
    try { const [profiles, library] = await Promise.all([api.profiles(), api.library()]); setVersions(profiles); setWorks(library); setSelectedVersion(profiles[0]?.version ?? null); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Profil indisponible."); }
    finally { setLoading(false); }
  }
  useEffect(() => { void load(); }, []);
  useEffect(() => { if (selectedVersion === null) return; api.historicalRecommendations(selectedVersion).then((result) => setHistory(result.recommendations)).catch(() => setHistory([])); }, [selectedVersion]);
  const selected = versions.find((version) => version.version === selectedVersion) ?? null;
  const workNames = useMemo(() => new Map(works.map((entry) => [entry.work.id, entry.work.title])), [works]);
  if (loading) return <div className="profile-state"><StatePanel eyebrow="Profil" title="La carte se charge." busy>Lecture des pôles et de leur évolution.</StatePanel></div>;
  if (error) return <div className="profile-state"><StatePanel eyebrow="Profil indisponible" title="La carte reste enregistrée." action={<Button tone="quiet" onClick={() => void load()}>Réessayer</Button>}>{error}</StatePanel></div>;
  if (!selected) return <div className="profile-state"><StatePanel eyebrow="Profil vide" title="Notez une première œuvre.">Votre première note créera une version consultable du profil.</StatePanel></div>;
  const positive = selected.profile.tag_affinities.filter((tag) => tag.value > 0).sort((a,b) => b.value - a.value).slice(0, 5);
  const negative = selected.profile.tag_affinities.filter((tag) => tag.value < 0).sort((a,b) => a.value - b.value).slice(0, 5);
  return <>
    <header className="page-header profile-hero"><div><p className="eyebrow">Profil de goût / version {selected.version}</p><h1>Ce que vos choix racontent.</h1><p className="page-header__intro">Une carte vérifiable : ses repères, son niveau de recul et chaque changement qui l’a façonnée.</p></div><span className="profile-confidence"><strong>{confidenceWord(selected.profile.confidence)}</strong><small>confiance · {selected.profile.history_size} œuvre{selected.profile.history_size > 1 ? "s" : ""}</small></span></header>
    <section className="profile-map" aria-labelledby="map-title"><header><div><p className="eyebrow">Carte actuelle</p><h2 id="map-title">Vos pôles</h2></div><p>{selected.profile.mode === "clustered" ? "Des familles distinctes émergent de vos favoris." : "Le profil reste regroupé tant que l’historique est court."}</p></header>
      <div className="profile-map__canvas" aria-hidden="true">{selected.profile.poles.map((pole, index) => <div className="profile-pole" key={pole.ordinal} style={{ "--pole-x": `${18 + (index * 31) % 68}%`, "--pole-y": `${30 + (index * 23) % 46}%` } as React.CSSProperties}><i /><strong>Pôle {pole.ordinal + 1}</strong><small>{pole.dominant_tags.slice(0, 3).map((tag) => tag.name).join(" · ")}</small></div>)}</div>
      <div className="profile-map__text"><h3>Alternative textuelle de la carte</h3>{selected.profile.poles.map((pole) => <article key={pole.ordinal}><strong>Pôle {pole.ordinal + 1} · {pole.member_count} œuvre{pole.member_count > 1 ? "s" : ""}</strong><p>Dominantes : {pole.dominant_tags.map((tag) => tag.name).join(", ")}.</p><p>Repères : {pole.representative_work_ids.map((id) => workNames.get(id) ?? `œuvre ${id}`).join(", ")}.</p></article>)}</div>
    </section>
    <section className="profile-details"><div className="axis-panel"><p className="eyebrow">Sensibilités déclarées</p><h2>Ce qui compte</h2>{selected.profile.axes.weights.map((axis) => <div className="axis-row" key={axis.axis}><span>{axisLabels[axis.axis] ?? axis.axis}</span><i><b style={{ width: `${axis.weight * 100}%` }} /></i><small>{selected.profile.axes.source === "learned" ? "appris" : "à confirmer"}</small></div>)}</div>
      <div className="affinity-panel"><div><p className="eyebrow">Affinités positives</p>{positive.length ? positive.map((tag) => <span key={tag.name}>{tag.name}</span>) : <p>Pas encore assez de contraste.</p>}</div><div><p className="eyebrow">Affinités négatives</p>{negative.length ? negative.map((tag) => <span className="negative" key={tag.name}>{tag.name}</span>) : <p>Aucun rejet appris pour le moment.</p>}</div></div></section>
    <section className="history-section" aria-labelledby="history-title"><header className="decision-heading"><div><p className="eyebrow">Évolution inspectable</p><h2 id="history-title">Versions du profil</h2></div><p>Sélectionnez une étape pour relire les recommandations conservées à cet instant.</p></header><div className="history-layout"><ol className="version-timeline">{versions.map((version) => <li key={version.version}><button className={version.version === selectedVersion ? "is-current" : ""} onClick={() => setSelectedVersion(version.version)} aria-pressed={version.version === selectedVersion}><i /><span><strong>Version {version.version}</strong><small>{new Date(version.created_at_unix * 1000).toLocaleString("fr-FR", { dateStyle: "medium", timeStyle: "short" })}</small></span><em>{version.profile.history_size} œuvres</em></button></li>)}</ol><div className="history-recommendations"><p className="eyebrow">Recommandations conservées · v{selectedVersion}</p>{history.length ? history.slice(0, 5).map((item) => <article key={item.work_id}><strong>{item.title}</strong><small>{item.explanation.reasons[0]?.detail ?? "Aucune raison dominante"}</small></article>) : <p>Aucune recommandation n’était disponible dans cette version.</p>}</div></div></section>
  </>;
}

function confidenceWord(value: number) { if (value >= .66) return "Solide"; if (value >= .3) return "En construction"; return "Préliminaire"; }
