# LivingDocs — Implementation Plan

> **Audience:** a Claude Sonnet coding agent implementing this repo.
> **Source of truth for *what* to build:** [CLAUDE.md](CLAUDE.md). This file is *how* and *in what order*.
> Build phases top to bottom. Each phase is independently testable and leaves the tool in a working state. Do not start a phase until the previous one's acceptance criteria pass.

---

## 0. Ground Rules (decided — do not re-litigate)

* **Form factor:** a CLI named `livingdocs` — a single statically-linked binary, distributed via `cargo install`, Homebrew, GitHub Releases, and an npm wrapper that fetches the prebuilt binary.
* **Language/runtime:** Rust (2021 edition). `tokio` for async (OpenAI / IO); keep everything else synchronous. Deny warnings in CI; `clippy` clean.
* **Hero feature:** drift detection (`livingdocs check`). Build it before anything that calls an LLM.
* **Local vs cloud split:** scanning, parsing, the graph, and drift detection run **locally with no model**. Only *synthesis* (prose/diagrams) calls **OpenAI**. Send graph facts, never raw source files.
* **Non-negotiable invariants** (assert these in tests):
  1. `livingdocs check` makes **zero network calls**. It is pure graph math.
  2. Generated output is **deterministic** — same graph in → byte-identical Markdown out. No timestamps or model chatter inside managed blocks.
  3. Writes go to a **branch + PR**, never a silent push to `main`.
  4. Every command is **non-interactive** and returns stable **exit codes** (`0` clean, `1` drift found, `2` error).
* **Scope discipline:** MVP languages are TypeScript + JavaScript only. Java/Python and framework route-extractors are Phase 8. Do not add them earlier.

---

## 1. Locked Tech Stack

| Concern | Crate |
| --- | --- |
| CLI parsing | `clap` (derive) |
| Async runtime | `tokio` (only where needed: OpenAI + IO) |
| Parsing | `tree-sitter` + `tree-sitter-typescript`, `tree-sitter-javascript` |
| Graph storage | `rusqlite` (with `bundled` feature) |
| In-memory graph | `petgraph` |
| LLM | `async-openai`; default `gpt-4.1`, bulk `gpt-4o-mini` |
| File walking | `ignore` (gitignore-aware) + `globset` |
| Serde | `serde`, `serde_json` (config + manifest), `serde_yaml` (front matter) |
| Markdown | `pulldown-cmark` to parse; hand-written managed-block parser (don't pull a heavy dep) |
| Git/PR | `git2` (libgit2) for branch/commit; `gh` CLI (or `octocrab`) for PR |
| Hashing | `blake3` (block + slice content hashes) |
| Errors | `anyhow` (bin) + `thiserror` (library modules) |
| Logging | `tracing` + `tracing-subscriber` |
| Test | `cargo test` + `assert_cmd` + `predicates` + `insta` (snapshots) |
| Lint/format | `clippy` + `rustfmt` |

---

## 2. Target Source Layout

```text
Cargo.toml                       # bin "livingdocs"; edition 2021
src/
├── main.rs                      # clap parse → dispatch to a command module
├── cli/
│   ├── mod.rs
│   ├── init.rs  analyze.rs  check.rs  update.rs
│   ├── sync.rs  watch.rs    explain.rs review.rs
├── config.rs                    # serde load + validate livingdocs.config.json
├── scanner.rs                   # file discovery (ignore + globset, git-diff aware)
├── parser/
│   ├── mod.rs                   # parse file → extracted nodes
│   └── typescript.rs
├── graph/
│   ├── mod.rs
│   ├── schema.sql               # embedded via include_str!
│   ├── db.rs                    # rusqlite open/migrate
│   ├── build.rs                 # nodes → rows; stable symbol ids
│   └── queries.rs               # deps, consumers, cycles (petgraph)
├── drift/
│   ├── mod.rs  findings.rs      # finding model + text/json formatters
│   └── rules/                   # one module per rule
├── docs/
│   ├── mod.rs  manifest.rs  frontmatter.rs  managed_blocks.rs
│   └── templates/               # overview, architecture, component, api, diagram
├── synthesis/
│   ├── mod.rs  provider.rs  openai.rs   # Provider trait + impl
│   ├── prompts.rs  cache.rs              # graph-slice prompts; hash-keyed cache
├── output/git.rs               # branch, commit managed files, open PR
└── util/{hash.rs, log.rs, exit.rs}
tests/                           # integration tests (assert_cmd); fixtures/ under here
.github/workflows/livingdocs.yml # shipped template for scheduled runs
```

---

## 3. Phased Build

### Phase 0 — Scaffold
**Tasks**
- `cargo init` a binary crate; `Cargo.toml` with the `livingdocs` bin, edition 2021, and the §1 crates; add `rustfmt.toml` and `clippy` in CI (`-D warnings`).
- Wire `clap` with all subcommands as stubs that print "not implemented" and exit `2`.
- Implement `config.rs` (serde-deserialize + validate `livingdocs.config.json`; sensible defaults; clear error on bad config).
- `util/exit.rs` (exit-code constants), `util/log.rs` (`tracing`; quiet by default, `--verbose`).

**Done when:** `cargo run -- --help` lists every command; `--version` works; `livingdocs init` writes a default `livingdocs.config.json` and `docs/` scaffold.

---

### Phase 1 — Scanner + Parser (TS/JS)
**Tasks**
- `scanner.rs`: walk repo with `ignore` honoring `include`/`exclude`; expose `scan_all()` and `scan_diff(since_commit)` (via `git2`).
- `parser`: load the `tree-sitter` grammars; extract `ClassNode`, `FunctionNode`, `InterfaceNode`, `ImportNode` with source ranges.

**Done when:** `livingdocs analyze --dry-run` prints extracted symbols as JSON for a fixture repo. Unit tests cover each node type.

---

### Phase 2 — Knowledge Graph
**Tasks**
- `graph/schema.sql`: `files, symbols, dependencies, imports, api_routes, documentation, relationships`. Stable `symbol_id` (e.g. `blake3` of `path#qualifiedName`, *not* line number, so renames/moves are tractable).
- `graph/build.rs`: parsed nodes → rows (`rusqlite`); resolve imports → dependency edges.
- `graph/queries.rs`: `dependencies_of`, `consumers_of` (reverse edges), `cycles()` (via `petgraph`).
- Wire `livingdocs analyze` to build `.livingdocs/graph.db`.

**Done when:** `analyze` populates `graph.db`; a query test returns correct deps and consumers for a fixture; `cycles()` detects a seeded circular dep.

---

### Phase 3 — Drift Detection ⭐ (hero; still no LLM)
**Tasks**
- `docs/frontmatter.rs` + `docs/managed_blocks.rs`: parse front matter and `<!-- LIVINGDOCS:BEGIN id=.. hash=.. -->` blocks.
- `docs/manifest.rs`: read/write `.livingdocs/manifest.json` (block → entity/source/hash).
- `drift/rules/`: implement rules — missing entity, removed/renamed route, gone symbol, dependency named in a doc but absent from graph, manual edit of a managed block (hash mismatch).
- `drift/findings.rs`: `{file, line, rule, severity, message}` + `text` and `json` formatters.
- Wire `livingdocs check`: walk manifest + docs against graph; emit findings; exit `1` if any, else `0`.

**Done when:** fixture `tests/fixtures/drifted` → `check` reports the seeded findings and exits `1`; `tests/fixtures/clean` exits `0`. **Test asserts zero network calls during `check`.** This phase alone is a shippable, useful tool.

---

### Phase 4 — Synthesis Adapter (OpenAI)
**Tasks**
- `synthesis/provider.rs`: `Provider` trait (`async fn synthesize(&self, prompt, opts)`); `openai.rs` impl over `async-openai`.
- `synthesis/prompts.rs`: build prompts from **graph slices** (entity + methods + deps as JSON), never raw files.
- `synthesis/cache.rs`: cache by `blake3` of the graph slice; skip the call on hit.
- Enforce `budget` (max tokens / max findings per run); model selection (default vs bulk).

**Done when:** given a graph slice, returns a component summary; a second run with unchanged slice is a cache hit (no API call). The `Provider` trait is mockable in tests so nothing downstream needs real OpenAI.

---

### Phase 5 — Doc Generator + Sync Engine
**Tasks**
- `docs/templates/`: deterministic renderers for `overview`, `architecture`, `components/<slug>`, `apis/<resource>`, `dependencies`, `diagrams/*` (Mermaid). Stable ordering everywhere.
- Managed-block writer: insert/replace blocks by `id`, recompute `hash`, update manifest + front matter. Never touch content outside markers.
- `livingdocs sync`: regenerate all managed sections from the graph.
- `livingdocs update`: the loop → `analyze` (diff) → `check` → synthesize **only drifted** nodes → write → **re-run `check` to verify** → report.

**Done when:** `analyze && sync` produces the full `docs/` tree from §"Generated Documentation Structure" in CLAUDE.md; `update` rewrites only drifted blocks; **running `sync` twice produces no git diff** (determinism test); hand-written content outside markers survives a sync.

---

### Phase 6 — Git Output + PR
**Tasks**
- `output/git.rs`: create/reset `output.branch` (`git2`), commit only managed-doc changes, open a PR via `gh` (fallback: print branch + instructions if `gh` absent).
- `--pr` flag on `update`/`sync`. Guard: refuse to write to `main`/`master` directly.

**Done when:** `update --pr` on a fixture repo creates a branch and a PR containing the doc changes; without `--pr` it writes to the working tree only.

---

### Phase 7 — Watch, Explain, Review, Scheduling
**Tasks**
- `livingdocs watch`: debounced re-`check` on file changes (local dev).
- `livingdocs explain <name>`: grounded answer from a graph slice (the demoted "chat").
- `livingdocs review`: cycles, oversized modules, tight coupling from graph metrics.
- Ship `.github/workflows/livingdocs.yml` (scheduled `cron` → `livingdocs update --pr`) and document git-hook usage.

**Done when:** `watch` re-checks on save; `explain` returns a grounded answer; `review` flags a seeded cycle; the sample Action runs `update --pr` end-to-end in CI.

---

### Phase 8 — Breadth & Hardening (post-MVP)
- Add Java + Python grammars and framework route-extractors (Express, Fastify, Spring, NestJS) behind the existing pluggable parser interface.
- Confidence scoring + `<!-- livingdocs:ignore -->` suppression on drift findings.
- Eval harness: `test/corpus/` of repos with known drift; measure precision/recall on `check` and quality on synthesis.

---

## 4. Cross-Cutting Requirements (every phase)

- **Tests first** for parser, graph queries, and drift rules — they are pure functions over fixtures and cheap to test.
- **No OpenAI in `check`** — structure so the `check` path never constructs the synthesis client; enforce with an `assert_cmd` test (run with no `OPENAI_API_KEY` and confirm success).
- **Determinism** — an `insta` snapshot test that runs generation twice and diffs the output.
- **Exit codes** — covered by `assert_cmd` integration tests.
- **Fixtures** — keep small sample repos under `tests/fixtures/` (`clean`, `drifted`, `cyclic`).

---

## 5. First Demoable Milestone

End of **Phase 3**: `livingdocs init && livingdocs analyze && livingdocs check` finds real stale docs in a repo, with no API key required. That proves the core thesis before a single OpenAI call. Prioritize reaching it.

---

## 6. Out of Scope (for now)

- VS Code extension (future client over the same engine).
- Multi-repo / microservice landscape mapping.
- Git-history understanding and ADR generation (roadmap V2).
- Any provider other than OpenAI (the interface allows it later; don't build it now).
