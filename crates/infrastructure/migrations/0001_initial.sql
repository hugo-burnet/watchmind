PRAGMA foreign_keys = ON;

CREATE TABLE works (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    payload TEXT NOT NULL CHECK (json_valid(payload))
);
CREATE TABLE tags (
    work_id INTEGER NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE CHECK (trim(name) <> ''),
    weight REAL NOT NULL CHECK (weight > 0 AND weight <= 1),
    PRIMARY KEY (work_id, name)
);
CREATE TABLE ratings (
    work_id INTEGER PRIMARY KEY REFERENCES works(id) ON DELETE CASCADE,
    rating REAL NOT NULL CHECK (rating >= 0 AND rating <= 10)
);
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    work_id INTEGER NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('completed', 'dropped', 'rewatched')),
    position INTEGER,
    total INTEGER,
    CHECK ((kind = 'dropped' AND position >= 0 AND total > 0 AND position < total)
        OR (kind <> 'dropped' AND position IS NULL AND total IS NULL))
);
CREATE TABLE aspects (
    work_id INTEGER NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    axis TEXT NOT NULL CHECK (axis IN ('story', 'characters', 'world_building', 'visual_direction', 'sound_and_music')),
    credit REAL NOT NULL CHECK (credit > 0 AND credit <= 1),
    PRIMARY KEY (work_id, axis)
);
CREATE TABLE preferences (
    key TEXT PRIMARY KEY CHECK (trim(key) <> ''),
    value TEXT NOT NULL CHECK (json_valid(value))
);
