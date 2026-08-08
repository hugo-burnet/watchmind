import { useState } from "react";
import { Button, StatePanel } from "./components/Primitives";
import { TasteMap } from "./components/TasteMap";

const navItems = ["Aujourd’hui", "Bibliothèque", "Carte de goût", "Évaluation"];

export function App() {
  const [active, setActive] = useState("Carte de goût");

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main">Aller au contenu</a>
      <aside className="sidebar">
        <a className="brand" href="#main" aria-label="WatchMind, accueil">
          <span className="brand__frame" aria-hidden="true"><i /></span>
          <span>Watch<br />Mind</span>
        </a>

        <nav aria-label="Navigation principale">
          {navItems.map((item) => (
            <button
              key={item}
              className={active === item ? "nav-item nav-item--active" : "nav-item"}
              onClick={() => setActive(item)}
              aria-current={active === item ? "page" : undefined}
            >
              <span>{item}</span>
              {item === "Aujourd’hui" && <em>2</em>}
            </button>
          ))}
        </nav>

        <div className="sidebar__foot">
          <span className="sync-dot" />
          <p><strong>Profil à jour</strong><small>12 œuvres · il y a 4 min</small></p>
        </div>
      </aside>

      <main id="main" className="main-content">
        <header className="page-header">
          <div>
            <p className="eyebrow">Laboratoire visuel / lot 15</p>
            <h1>Comprendre avant de choisir.</h1>
            <p className="page-header__intro">
              Une lecture personnelle de vos goûts, dessinée à partir de ce que vous avez vraiment aimé.
            </p>
          </div>
          <Button tone="quiet">Ajouter une œuvre <span aria-hidden="true">＋</span></Button>
        </header>

        <TasteMap />

        <section className="lab" aria-labelledby="lab-title">
          <header className="section-heading">
            <div>
              <p className="eyebrow">Primitives système</p>
              <h2 id="lab-title">Des états qui indiquent la suite.</h2>
            </div>
            <p>Chaque message donne une cause claire et une action possible.</p>
          </header>

          <div className="state-grid">
            <StatePanel eyebrow="Bibliothèque vide" title="Commencez par trois œuvres.">
              Ajoutez quelques repères pour dessiner votre première carte.
            </StatePanel>
            <StatePanel eyebrow="Calcul en cours" title="Les chemins se redessinent." busy>
              Vos notes restent disponibles pendant la mise à jour.
            </StatePanel>
            <StatePanel
              eyebrow="Catalogue indisponible"
              title="AniList ne répond pas."
              action={<Button tone="quiet">Réessayer</Button>}
            >
              La dernière carte enregistrée reste consultable hors ligne.
            </StatePanel>
          </div>
        </section>
      </main>
    </div>
  );
}
