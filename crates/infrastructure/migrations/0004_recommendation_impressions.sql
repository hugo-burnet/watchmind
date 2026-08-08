-- Journal de ce qui a ete REELLEMENT affiche a l'utilisateur.
--
-- Le leave-one-out ne peut pas dire si le moteur est utile : il oppose une
-- oeuvre choisie et adoree a des oeuvres jamais touchees, si bien que tout
-- indicateur de notoriete gagne d'avance. Seule la trace de ce qui a ete montre,
-- croisee avec ce qui a ete regarde ensuite, mesure l'apport reel du moteur.
CREATE TABLE recommendation_impressions (
    work_id INTEGER NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    profile_version INTEGER NOT NULL,
    shown_at_unix INTEGER NOT NULL CHECK (shown_at_unix >= 0),
    rank INTEGER NOT NULL CHECK (rank > 0),
    global_score REAL CHECK (global_score IS NULL OR (global_score >= 0 AND global_score <= 10)),
    PRIMARY KEY (work_id, profile_version)
);

CREATE INDEX recommendation_impressions_by_date
    ON recommendation_impressions(shown_at_unix);
