# tantivy-demo

A minimal Elasticsearch-like search service in Rust using Tantivy and Actix-web.

Highlights
- HTTP JSON API for indexing and searching
- Real JSON field for `features` with nested queries (e.g., `features.lang:zh`)
- Per-field analyzers:
  - `title`, `body`: CJK-friendly n-gram analyzer (`zh_ngram`, 2-3 char grams + lowercase)
  - `tags`: whitespace + lowercase analyzer (`whitespace_lc`)
- Concurrent, hot-reloadable searchers with periodic commits
- Simple update/delete by unique id
- Two CLI tools for load generation and querying

Schema
- id: STRING, stored
- title: TEXT, stored (analyzer: `zh_ngram`)
- body: TEXT, stored (analyzer: `zh_ngram`)
- tags: TEXT, stored (analyzer: `whitespace_lc`)
- create_at: i64, stored
- status: STRING, stored
- features: JSON, stored + indexed for nested queries

Run the service
- cargo run --bin tantivy-demo
- Server: http://127.0.0.1:8080
- Index path: .tantivy_idx (created alongside the binary)

Endpoints (curl examples)
1) Index one document
curl -X POST http://127.0.0.1:8080/index -H "Content-Type: application/json" -d '{
  "id":"1",
  "title":"Rust 搜尋",
  "body":"使用 Tantivy 打造搜尋服務",
  "tags":["rust","search"],
  "create_at": 1734050000,
  "status":"published",
  "features":{"lang":"zh","length":123}
}'

2) Update (delete by id then re-index)
curl -X POST http://127.0.0.1:8080/update -H "Content-Type: application/json" -d '{
  "id":"1",
  "title":"Rust search UPDATED",
  "body":"Updated body",
  "tags":["rust","search"],
  "create_at": 1734050001,
  "status":"published",
  "features":{"lang":"en","length":456}
}'

3) Delete by id
curl -X DELETE "http://127.0.0.1:8080/delete?id=1"

4) Search (default fields: title, body, tags, features)
- Full text: curl "http://127.0.0.1:8080/search?q=rust&limit=5"
- Nested JSON: curl "http://127.0.0.1:8080/search?q=features.lang:zh&limit=5"
- Field-scoped: curl "http://127.0.0.1:8080/search?q=title:搜索&limit=5"

CLI tools
- Generator (concurrent indexing of synthetic data)
  cargo run --bin generate -- --count 5000 --concurrency 16 --endpoint http://127.0.0.1:8080

- Search client (queries the HTTP API and prints JSON)
  cargo run --bin search -- --q "features.lang:en" --limit 10 --endpoint http://127.0.0.1:8080

Analyzers
- zh_ngram: registered as 2–3 character n-grams + lowercase; good baseline for CJK without external deps
- whitespace_lc: whitespace + lowercase tokenizer for tags-like fields
- To switch to jieba or other tokenizers, register them and update TextOptions per field

Implementation notes
- Background commit + reader reload every 3s
- Searcher is hot-swapped with ArcSwap for consistent low-latency reads while indexing
- Writer protected by Mutex for safe mutation
- Update uses delete-by-term (id) then add

Limitations / Future work
- Search response currently returns Tantivy debug values; consider mapping back to a clean BlogPost JSON
- Add bulk indexing endpoint and health endpoint
- Add generator options for Chinese content ratio and language pools
- Fine-tune analyzers per field; add language detection or per-doc analyzer routing
