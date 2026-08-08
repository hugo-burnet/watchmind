import { Confidence } from "./Primitives";

const nodes = [
  { id: "monster", title: "Monster", note: "9,4", x: 18, y: 31, kind: "source" },
  { id: "lain", title: "Serial Experiments Lain", note: "9,1", x: 29, y: 73, kind: "source" },
  { id: "perfect", title: "Perfect Blue", note: "9,0", x: 53, y: 20, kind: "source" },
  { id: "pluto", title: "Pluto", note: "À voir", x: 71, y: 45, kind: "target" },
  { id: "odd", title: "Odd Taxi", note: "À voir", x: 80, y: 77, kind: "target" },
] as const;

export function TasteMap() {
  return (
    <section className="taste-map" aria-labelledby="map-title">
      <header className="taste-map__header">
        <div>
          <p className="eyebrow">Carte de goût · profil v12</p>
          <h2 id="map-title">Vos affinités ouvrent deux chemins.</h2>
        </div>
        <div className="map-legend" aria-label="Légende">
          <span><i className="legend-dot legend-dot--source" />Œuvre aimée</span>
          <span><i className="legend-dot legend-dot--target" />À découvrir</span>
        </div>
      </header>

      <div className="taste-map__canvas">
        <svg className="taste-map__routes" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <path className="route route--steady" d="M18 31 C34 28 49 38 71 45" />
          <path className="route route--steady" d="M53 20 C59 25 64 34 71 45" />
          <path className="route route--bet" d="M29 73 C45 64 63 71 80 77" />
          <path className="route route--bet" d="M18 31 C35 49 55 63 80 77" />
        </svg>
        <span className="contour contour--one" aria-hidden="true" />
        <span className="contour contour--two" aria-hidden="true" />

        {nodes.map((node) => (
          <button
            key={node.id}
            className={`map-node map-node--${node.kind}`}
            style={{ left: `${node.x}%`, top: `${node.y}%` }}
            aria-label={`${node.title}, ${node.note}`}
          >
            <span className="map-node__pin" aria-hidden="true" />
            <span className="map-node__label">
              <strong>{node.title}</strong>
              <small>{node.note}</small>
            </span>
          </button>
        ))}

        <div className="map-callout">
          <p className="eyebrow">Chemin le plus sûr</p>
          <strong>Pluto</strong>
          <p>Thriller moral, tension lente et personnages ambigus.</p>
          <Confidence value={86} />
        </div>
      </div>
    </section>
  );
}
