import { FormEvent, useEffect, useMemo, useState } from "react";
import { api, type CompleteWork, type Work } from "../api";
import { Button, StatePanel } from "./Primitives";

const aspectOptions = [
  ["story", "Récit"], ["characters", "Personnages"], ["visual_direction", "Mise en scène"],
  ["sound_and_music", "Son & musique"], ["world_building", "Univers"],
] as const;

function meta(work: Work) {
  return [work.release_year, work.format?.replaceAll("_", " "), work.runtime_minutes && `${work.runtime_minutes} min`]
    .filter(Boolean).join(" · ");
}

export function Library() {
  const [entries, setEntries] = useState<CompleteWork[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [filter, setFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Work[]>([]);
  const [searching, setSearching] = useState(false);
  const [selected, setSelected] = useState<CompleteWork | null>(null);
  const [confirmingRemoval, setConfirmingRemoval] = useState<number | null>(null);
  const [removing, setRemoving] = useState<number | null>(null);

  async function load() {
    setLoading(true); setError("");
    try { setEntries(await api.library()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Bibliothèque indisponible."); }
    finally { setLoading(false); }
  }

  useEffect(() => { void load(); }, []);

  async function search(event: FormEvent) {
    event.preventDefault();
    if (!query.trim()) return;
    setSearching(true); setError("");
    try { setResults((await api.search(query.trim())).works); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Recherche indisponible."); }
    finally { setSearching(false); }
  }

  async function add(work: Work) {
    setError("");
    try { await api.add(work); setResults((current) => current.filter((item) => item.id !== work.id)); await load(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Ajout impossible."); }
  }

  async function remove(entry: CompleteWork) {
    setRemoving(entry.work.id); setError("");
    try { await api.remove(entry.work.id); setConfirmingRemoval(null); await load(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "Suppression impossible."); }
    finally { setRemoving(null); }
  }

  const visible = useMemo(() => entries.filter((entry) => {
    if (filter === "rated") return entry.rating;
    if (filter === "unrated") return !entry.rating;
    if (filter === "dropped") return entry.events.some((event) => event.kind === "dropped");
    return true;
  }), [entries, filter]);

  return <>
    <header className="page-header library-hero">
      <div>
        <p className="eyebrow">Bibliothèque / lot 16</p>
        <h1>Trouver. Voir. Se souvenir.</h1>
        <p className="page-header__intro">Une trace précise de ce que vous regardez et de ce qui mérite de rester.</p>
      </div>
      <span className="library-count"><strong>{entries.length}</strong> œuvres repères</span>
    </header>

    <section className="search-deck" aria-labelledby="search-title">
      <div><p className="eyebrow">Catalogue AniList</p><h2 id="search-title">Ajouter une œuvre</h2></div>
      <form className="search-form" role="search" onSubmit={search}>
        <label className="sr-only" htmlFor="catalog-search">Titre de l’œuvre</label>
        <input id="catalog-search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Ex. Cowboy Bebop" />
        <Button type="submit" disabled={searching}>{searching ? "Recherche…" : "Rechercher"}</Button>
      </form>
      {results.length > 0 && <div className="search-results" aria-label="Résultats de recherche">
        {results.map((work) => <article key={work.id} className="search-result">
          <div className="work-glyph" aria-hidden="true">{work.title.slice(0, 2)}</div>
          <div><strong>{work.title}</strong><small>{meta(work) || "Informations AniList"}</small></div>
          <Button tone="quiet" onClick={() => void add(work)}>Ajouter <span aria-hidden="true">+</span></Button>
        </article>)}
      </div>}
    </section>

    {error && <div className="inline-error" role="alert"><span>{error}</span><Button tone="quiet" onClick={() => void load()}>Réessayer</Button></div>}

    <section className="library-section" aria-labelledby="library-title">
      <header className="library-toolbar">
        <div><p className="eyebrow">Collection personnelle</p><h2 id="library-title">Ma bibliothèque</h2></div>
        <div className="filter-group" aria-label="Filtrer la bibliothèque">
          {[["all", "Toutes"], ["rated", "Notées"], ["unrated", "À noter"], ["dropped", "Arrêtées"]].map(([value, label]) =>
            <button key={value} className={filter === value ? "filter is-active" : "filter"} onClick={() => setFilter(value)} aria-pressed={filter === value}>{label}</button>)}
        </div>
      </header>
      {loading ? <div className="state-grid single"><StatePanel eyebrow="Bibliothèque" title="Les œuvres arrivent." busy>Lecture de votre collection locale.</StatePanel></div>
      : visible.length === 0 ? <div className="state-grid single"><StatePanel eyebrow="Aucun résultat" title="Ce rayon est encore vide.">Recherchez une œuvre ou choisissez un autre filtre.</StatePanel></div>
      : <div className="work-grid">{visible.map((entry, index) => <article className="work-card" key={entry.work.id}>
          <button className="work-card__open" onClick={() => setSelected(entry)} aria-label={`Ouvrir la fiche de ${entry.work.title}`}>
            <span className="work-card__index">{String(index + 1).padStart(2, "0")}</span>
            <span className="work-card__body"><small>{meta(entry.work) || "Œuvre AniList"}</small><strong>{entry.work.title}</strong>
              <span className="tag-row">{entry.work.tags.slice(0, 2).map((tag) => <i key={tag.name}>{tag.name}</i>)}</span></span>
            <span className={entry.rating ? "personal-rating" : "personal-rating is-empty"}>{entry.rating ? <><b>{entry.rating.rating}</b>/10</> : "À noter"}</span>
          </button>
          <div className="card-removal">{confirmingRemoval === entry.work.id ? <><span>Retirer cette œuvre et ses données actives ?</span><button onClick={() => void remove(entry)} disabled={removing === entry.work.id}>{removing === entry.work.id ? "Suppression…" : "Confirmer"}</button><button onClick={() => setConfirmingRemoval(null)}>Annuler</button></> : <button onClick={() => setConfirmingRemoval(entry.work.id)} aria-label={`Supprimer ${entry.work.title} de la bibliothèque`}>Supprimer</button>}</div>
        </article>)}</div>}
    </section>
    {selected && <WorkSheet entry={selected} onClose={() => setSelected(null)} onRefresh={load} onSaved={async () => { await load(); setSelected(null); }} />}
  </>;
}

function WorkSheet({ entry, onClose, onRefresh, onSaved }: { entry: CompleteWork; onClose: () => void; onRefresh: () => Promise<void>; onSaved: () => Promise<void> }) {
  const [rating, setRating] = useState(entry.rating?.rating ?? 7);
  const [aspects, setAspects] = useState<string[]>(entry.rating?.aspects.map((item) => item.axis) ?? []);
  const [comment, setComment] = useState(entry.library?.comment ?? "");
  const dropped = entry.events.find((event) => event.kind === "dropped");
  const [status, setStatus] = useState(dropped ? "dropped" : entry.events.some((event) => event.kind === "completed") ? "completed" : "watching");
  const [position, setPosition] = useState(dropped?.progress?.position ?? 1);
  const [total, setTotal] = useState(dropped?.progress?.total ?? 12);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [rewatchCount, setRewatchCount] = useState(entry.events.filter((event) => event.kind === "rewatched").length);
  const [rewatching, setRewatching] = useState(false);
  const [message, setMessage] = useState("");

  function toggleAspect(axis: string) { setAspects((current) => current.includes(axis) ? current.filter((item) => item !== axis) : current.length < 2 ? [...current, axis] : current); }
  async function save() {
    setSaving(true); setError("");
    try {
      await api.updateComment(entry.work, comment.trim() || null);
      await api.rate(entry.work.id, rating, aspects);
      if (status === "completed") await api.event(entry.work.id, { kind: "completed", work_id: entry.work.id });
      if (status === "dropped") await api.event(entry.work.id, { kind: "dropped", work_id: entry.work.id, progress: { position, total } });
      await onSaved();
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Enregistrement impossible."); }
    finally { setSaving(false); }
  }
  async function addRewatch() {
    setRewatching(true); setError(""); setMessage("");
    try {
      await api.event(entry.work.id, { kind: "rewatched", work_id: entry.work.id });
      setRewatchCount((count) => count + 1);
      setMessage("Rewatch enregistré.");
      await onRefresh();
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Rewatch impossible."); }
    finally { setRewatching(false); }
  }

  return <div className="sheet-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }} onKeyDown={(event) => { if (event.key === "Escape") onClose(); }}>
    <section className="work-sheet" role="dialog" aria-modal="true" aria-labelledby="sheet-title">
      <button className="sheet-close" onClick={onClose} aria-label="Fermer la fiche" autoFocus>×</button>
      <p className="eyebrow">Fiche œuvre · {meta(entry.work)}</p><h2 id="sheet-title">{entry.work.title}</h2>
      <div className="sheet-tags">{entry.work.tags.slice(0, 4).map((tag) => <span key={tag.name}>{tag.name}</span>)}</div>
      <fieldset><legend>Où en êtes-vous ?</legend><div className="segmented">
        {[["watching", "En cours"], ["completed", "Terminée"], ["dropped", "Arrêtée"]].map(([value, label]) => <button key={value} type="button" aria-pressed={status === value} onClick={() => setStatus(value)}>{label}</button>)}
      </div></fieldset>
      {status === "dropped" && <div className="drop-position"><label>Arrêt à <input type="number" min="1" max={Math.max(1, total - 1)} value={position} onChange={(event) => setPosition(Number(event.target.value))} /></label><span>sur</span><label><span className="sr-only">Nombre total d’épisodes</span><input type="number" min="2" value={total} onChange={(event) => setTotal(Number(event.target.value))} /> épisodes</label></div>}
      <fieldset className="rating-field"><legend>Votre note</legend><div><input type="range" min="0" max="10" step="0.5" value={rating} onChange={(event) => setRating(Number(event.target.value))} aria-valuetext={`${rating} sur 10`} /><output>{rating}<small>/10</small></output></div></fieldset>
      <fieldset><legend>Ce qui a compté <small>2 maximum</small></legend><div className="aspect-chips">{aspectOptions.map(([axis, label]) => <button type="button" key={axis} aria-pressed={aspects.includes(axis)} onClick={() => toggleAspect(axis)} disabled={!aspects.includes(axis) && aspects.length >= 2}>{label}</button>)}</div></fieldset>
      <label className="comment-field">Une phrase pour vous, si utile<textarea rows={3} maxLength={240} value={comment} onChange={(event) => setComment(event.target.value)} placeholder="Ce que vous voudrez vous rappeler…" /></label>
      {rewatchCount > 0 && <p className="rewatch-count">Revu {rewatchCount} fois</p>}
      {message && <p className="form-success" role="status">{message}</p>}
      {error && <p className="form-error" role="alert">{error}</p>}
      <div className="sheet-actions"><Button tone="quiet" onClick={() => void addRewatch()} disabled={rewatching}>{rewatching ? "Enregistrement…" : "Marquer un rewatch"}</Button><Button onClick={() => void save()} disabled={saving}>{saving ? "Enregistrement…" : "Enregistrer"}</Button></div>
    </section>
  </div>;
}
