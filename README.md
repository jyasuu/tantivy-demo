# tantivy-demo

A minimal Elasticsearch-like search service in Rust using Tantivy and Actix-web.

Features:
- JSON indexing via POST /index
- Full-text search via GET /search?q=...
- Concurrent reads with hot-swappable Searcher and periodic commits
- Schema: id (STRING), title (TEXT), body (TEXT), tags (TEXT), create_at (i64), status (STRING), features (TEXT serialized JSON)

Run:
- cargo run
- Server: http://127.0.0.1:8080

Index example:
- curl -X POST http://127.0.0.1:8080/index -H "Content-Type: application/json" -d '{
    "id":"1",
    "title":"Rust search",
    "body":"Using Tantivy to build a search service",
    "tags":["rust","search"],
    "create_at": 1734050000,
    "status":"published",
    "features":{"lang":"en","length":123}
  }'

Search example:
- curl "http://127.0.0.1:8080/search?q=rust&limit=5"

Update example:
- curl -X POST http://127.0.0.1:8080/update -H "Content-Type: application/json" -d '{
    "id":"1",
    "title":"Rust search UPDATED",
    "body":"Updated body",
    "tags":["rust","search"],
    "create_at": 1734050001,
    "status":"published",
    "features":{"lang":"en","length":456}
  }'

Delete example:
- curl -X DELETE "http://127.0.0.1:8080/delete?id=1"

CLI tools:
- cargo run --bin generate -- --count 1000 --concurrency 8 --endpoint http://127.0.0.1:8080
- cargo run --bin search -- --q "rust" --limit 10 --endpoint http://127.0.0.1:8080

Custom analyzers:
- Title/body use a CJK-friendly ngram analyzer (2-3 char grams) with lowercasing: tokenizer name "zh_ngram".
- Tags use a whitespace-lowercase analyzer: tokenizer name "whitespace_lc".
- Example Chinese query: curl "http://127.0.0.1:8080/search?q=标题:中文&limit=5"
- Nested JSON query: curl "http://127.0.0.1:8080/search?q=features.lang:zh&limit=5"
