# Elasticsearch-like Roadmap for tantivy-demo

A pragmatic plan to evolve this Tantivy + Actix-web service toward Elasticsearch-like capabilities while keeping complexity manageable.

## Current Capabilities (Baseline)
- Single index stored at `.tantivy_idx` with fixed schema:
  - id (string), title/body (CJK-friendly n-gram), tags (whitespace+lowercase), create_at (i64), status (string), features (JSON)
- HTTP endpoints:
  - POST /index — index one document
  - POST /update — delete-by-id then re-index
  - DELETE /delete?id=... — delete by id
  - GET /search?q=...&limit=... — query basic/default fields, JSON nested queries
- Infra:
  - Background commit + reader reload (~3s)
  - ArcSwap searcher; Mutex writer
- CLI:
  - generate — load generator
  - search — query helper

---

## Phase 1 — Core API Parity (MVP+)
Goal: Solid single-index service with familiar ES-like endpoints and behavior.

### Scope
- Index management
  - PUT /{index} — create index (initialize Tantivy directory + schema)
  - DELETE /{index} — delete index
  - GET /_cat/indices — list indices and basic stats

- Document CRUD
  - PUT /{index}/_doc/{id} — index a document
  - POST /{index}/_doc/{id} — same as PUT
  - GET /{index}/_doc/{id} — fetch stored doc by id
  - DELETE /{index}/_doc/{id} — delete by id

- Bulk API
  - POST /_bulk and/or /{index}/_bulk — NDJSON stream; supports index/delete/update (update = delete+index)

- Search API (subset of DSL)
  - POST /{index}/_search with:
    - Query: bool (must/should/filter), term/match/prefix, range (numeric/date), nested JSON (via Tantivy JSON)
    - Pagination: from/size
    - Sort: by fast fields
    - _source filtering: includes/excludes, fields selection

- Health and control
  - GET /_cluster/health — single-node status green/yellow/red
  - POST /{index}/_refresh — commit + reader reload
  - POST /{index}/_flush — force commit
  - GET /{index}/_stats — doc count, segments, size on disk, commit age

- Analyzers
  - POST /_analyze — run analyzer by name on input (supports zh_ngram, whitespace_lc)

### Acceptance Criteria
- Create/delete/list indices with persistent metadata; reload on restart.
- CRUD idempotency; GET returns stored fields; consistent after refresh.
- Bulk handles 10k+ ops per request with partial failure reporting.
- DSL-to-Tantivy translation covered by unit tests; stable sort/pagination; default field behavior matches current.
- Health endpoints reflect true writer/searcher state; safe refresh/flush during load.
- Analyze returns tokens for registered analyzers.

### Initial Tickets
- API/Infra
  - Index registry and per-index path layout
  - GET /_cluster/health, POST /{index}/_refresh, POST /{index}/_flush, GET /{index}/_stats
- Index management
  - PUT/DELETE /{index}, GET /_cat/indices
- CRUD
  - PUT/POST/GET/DELETE /{index}/_doc/{id}; store _source, set fast fields
- Bulk
  - Streaming NDJSON parser; chunked commits; error reporting; test with 100k ops
- Search
  - Minimal query DSL: match, term, bool (must/should/filter), range; from/size; sort by fast fields; _source filtering
- Analyze
  - POST /_analyze (zh_ngram, whitespace_lc)
- Tests and tooling
  - Integration tests; structured errors; extend generator for bulk

---

## Phase 2 — Query Capabilities and DX
Goal: Improve practical search quality and ergonomics.

### Scope
- Highlighting
  - Snippet generation via Tantivy SnippetGenerator; configurable fragment size/count

- Aggregations (initial)
  - terms, range, histogram, count (fast-field backed); custom collectors

- MSearch and Count
  - POST /_msearch — multiplex multiple searches
  - GET/POST /{index}/_count — count with optional query filter

- Delete by query
  - POST /{index}/_delete_by_query — translate query, collect ids/terms, batch delete, commit

- Mapping
  - GET /{index}/_mapping — export schema
  - PUT /{index}/_mapping — extend-only if possible; otherwise recreate guidance
  - Optional dynamic mapping (off by default) with type inference

- Errors and compatibility
  - Standardized error payloads similar to ES (type, reason, caused_by)

### Acceptance Criteria
- Highlighting with predictable performance; configurable and optional.
- Aggregations constrained to fast fields; documented limits and memory characteristics.
- Delete-by-query correctness with tests.
- Mapping retrieval stable; dynamic mapping guarded with versioning.

---

## Phase 3 — Multi-Index, Aliases, Admin
Goal: Multiple indices with routing and better ops.

### Scope
- Index aliases
  - PUT /_aliases — add/remove alias to index; resolve on read/write

- Templates and defaults
  - PUT /_index_template/{name} — bootstrap mappings/settings for new indices

- Snapshots and backup
  - Local snapshot of index dirs; metadata JSON; restore endpoint; optional S3-compatible storage

- Monitoring
  - GET /_nodes/stats — CPU/mem (process), writer queue depth, commit latency, query latency histograms
  - Prometheus metrics endpoint

- Rollovers
  - POST /{alias}/_rollover — create new index and switch alias on size/doc thresholds

### Acceptance Criteria
- Aliases routed reliably; rollover switches without downtime.
- Snapshot/restore validated; operational docs included.
- Metrics export compatible with Prometheus/Grafana.

---

## Phase 4 — Relevance, Scale, Security
Goal: Production quality for demanding use cases.

### Scope
- Relevance and ranking
  - BM25 tuning per field; field boosts; function_score-like numeric modifiers
  - Hooks for offline learning-to-rank

- Language handling
  - Analyzer registry per language; optional language detection; per-doc analyzer routing

- Scaling (single-node index)
  - Merge/IO tuning; memory/backpressure; writer timeouts; bulk admission control

- Security
  - API keys; role-based access (index-level); rate limits; audit logs; structured logs

- Stability and recovery
  - Commit journal/WAL checks; crash recovery tests; fsync strategy options

### Acceptance Criteria
- Documented relevance knobs; stable under failure; basic security in place.

---

## Timelines (Rough)
- Phase 1: 1–2 weeks
- Phase 2: 2–3 weeks
- Phase 3: 2–3 weeks
- Phase 4: 3–5 weeks
Adjust per team size and desired depth (especially aggregations/security).

---

## Key Risks and Mitigations
- Aggregations complexity
  - Start small; require fast fields; document limitations; test cardinality bounds
- Dynamic mapping conflicts
  - Default off; explicit mapping creation; versioned schemas
- Bulk memory pressure
  - Stream parse; chunked commits; backpressure on writer; admission control
- Highlighting performance
  - Limit fields and fragments; opt-in
- Snapshot consistency
  - Quiesce writer or checkpoint commits before snapshot; validate restore

---

## Nice-to-haves (Elasticsearch-aligned)
- _source includes/excludes
- track_total_hits (cap option)
- explain (developer mode only)
- profile (collector timing)
- search_after (deep pagination)
- terms enum endpoint (autocomplete)
