CREATE TABLE library (
    work_id INTEGER PRIMARY KEY REFERENCES works(id) ON DELETE CASCADE,
    comment TEXT
);

CREATE TABLE profile_snapshots (
    version INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at_unix INTEGER NOT NULL CHECK (created_at_unix >= 0),
    payload TEXT NOT NULL CHECK (json_valid(payload))
);

CREATE TABLE score_snapshots (
    profile_version INTEGER NOT NULL REFERENCES profile_snapshots(version) ON DELETE CASCADE,
    work_id INTEGER NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    rank INTEGER NOT NULL CHECK (rank > 0),
    payload TEXT NOT NULL CHECK (json_valid(payload)),
    PRIMARY KEY (profile_version, work_id)
);

CREATE INDEX score_snapshots_by_version_rank
    ON score_snapshots(profile_version, rank);
