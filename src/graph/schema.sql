-- LivingDocs knowledge graph. Rebuilt fresh on every `analyze` run (see
-- graph::db::open_fresh) so it is always a pure function of the current
-- repo state — never an accumulation of stale rows from past runs.

CREATE TABLE files (
    path TEXT PRIMARY KEY,
    language TEXT NOT NULL
);

CREATE TABLE symbols (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL REFERENCES files(path),
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    exported INTEGER NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    -- JSON array of method names; only set for kind = 'class'.
    methods_json TEXT
);
CREATE INDEX idx_symbols_file ON symbols(file_path);
CREATE INDEX idx_symbols_name ON symbols(name);

CREATE TABLE imports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL REFERENCES files(path),
    source TEXT NOT NULL,
    specifiers_json TEXT NOT NULL,
    start_line INTEGER NOT NULL
);
CREATE INDEX idx_imports_file ON imports(file_path);

-- Import specifiers resolved to a local file, one row per specifier.
-- Only relative imports ("./x", "../x") resolve here; bare package
-- specifiers (e.g. "express") stay in `imports` but never reach this
-- table — they're outside the local dependency graph.
CREATE TABLE dependencies (
    from_file TEXT NOT NULL REFERENCES files(path),
    to_file TEXT NOT NULL REFERENCES files(path),
    specifier TEXT NOT NULL,
    PRIMARY KEY (from_file, to_file, specifier)
);
CREATE INDEX idx_dependencies_to ON dependencies(to_file);

CREATE TABLE api_routes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL REFERENCES files(path),
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    handler TEXT,
    start_line INTEGER NOT NULL,
    framework TEXT NOT NULL
);
CREATE INDEX idx_api_routes_file ON api_routes(file_path);

-- Populated starting Phase 3: anchors a claim made in a human-written doc
-- (e.g. "docs/architecture.md:42 says Redis") to a graph fact, so `check`
-- can tell whether the claim still holds.
CREATE TABLE documentation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    doc_file TEXT NOT NULL,
    line INTEGER NOT NULL,
    kind TEXT NOT NULL,
    claim TEXT NOT NULL,
    entity TEXT
);

-- Populated starting Phase 3+: general-purpose edges beyond the
-- import-derived `dependencies` table (e.g. "exposes", "calls") as
-- richer analysis lands.
CREATE TABLE relationships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    kind TEXT NOT NULL
);
