use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix_web::{get, post, delete, web, App, HttpResponse, HttpServer, Responder};
use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, STORED, STRING, TEXT, OwnedValue, TextOptions, TextFieldIndexing, IndexRecordOption};
use tantivy::tokenizer::{TextAnalyzer, LowerCaser, WhitespaceTokenizer, NgramTokenizer};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, TantivyDocument, Term};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlogPost {
    pub id: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub create_at: Option<i64>,
    pub status: String,
    pub features: serde_json::Value,
}

pub struct AppState {
    pub writer: Arc<Mutex<IndexWriter>>,     // protected for add and commit
    pub reader: IndexReader,                  // used to get new searchers
    pub current_searcher: Arc<ArcSwap<Searcher>>, // hot-swapped searcher
}

fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // Per-field analyzers via TextOptions
    let zh_indexing = TextFieldIndexing::default()
        .set_tokenizer("zh_ngram")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let zh_text = TextOptions::default()
        .set_indexing_options(zh_indexing)
        .set_stored();

    let tags_indexing = TextFieldIndexing::default()
        .set_tokenizer("whitespace_lc")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let tags_text = TextOptions::default()
        .set_indexing_options(tags_indexing)
        .set_stored();

    schema_builder.add_text_field("id", STRING | STORED);
    schema_builder.add_text_field("title", zh_text.clone());
    schema_builder.add_text_field("body", zh_text);
    schema_builder.add_text_field("tags", tags_text);
    schema_builder.add_i64_field("create_at", STORED);
    schema_builder.add_text_field("status", STRING | STORED);
    schema_builder.add_json_field("features", TEXT | STORED);
    schema_builder.build()
}

fn to_document(schema: &Schema, post: BlogPost) -> TantivyDocument {
    let mut document = TantivyDocument::default();
    let f_id = schema.get_field("id").unwrap();
    let f_title = schema.get_field("title").unwrap();
    let f_body = schema.get_field("body").unwrap();
    let f_tags = schema.get_field("tags").unwrap();
    let f_create_at = schema.get_field("create_at").unwrap();
    let f_status = schema.get_field("status").unwrap();
    let f_features = schema.get_field("features").unwrap();

    document.add_text(f_id, post.id);
    document.add_text(f_title, post.title);
    document.add_text(f_body, post.body);
    for tag in post.tags.into_iter() {
        document.add_text(f_tags, tag);
    }
    if let Some(ts) = post.create_at {
        document.add_i64(f_create_at, ts);
    }
    document.add_text(f_status, post.status);
    let ov = OwnedValue::from(post.features);
    match ov {
        OwnedValue::Object(map) => {
            document.add_object(f_features, map);
        }
        other => {
            // Wrap non-object into an object under key "value" for JSON field
            let mut map = std::collections::BTreeMap::new();
            map.insert("value".to_string(), other);
            document.add_object(f_features, map);
        }
    }

    document
}

pub fn index_post(writer: &mut IndexWriter, schema: &Schema, post: BlogPost) -> tantivy::Result<u64> {
    let doc = to_document(schema, post);
    writer.add_document(doc)
}

#[post("/index")]
async fn add_document(data: web::Json<BlogPost>, state: web::Data<AppState>) -> impl Responder {
    let mut writer = match state.writer.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    };
    let schema = writer.index().schema();
    match index_post(&mut writer, &schema, data.into_inner()) {
        Ok(_) => HttpResponse::Ok().json("queued"),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct SearchQuery { q: String, limit: Option<usize> }

#[get("/search")]
async fn search_document(info: web::Query<SearchQuery>, state: web::Data<AppState>) -> impl Responder {
    let guard = state.current_searcher.load();
    let searcher: &Searcher = &guard;

    let index = searcher.index();
    let schema = index.schema();
    let default_fields = vec![
        schema.get_field("title").unwrap(),
        schema.get_field("body").unwrap(),
        schema.get_field("tags").unwrap(),
        schema.get_field("features").unwrap(),
    ];
    let parser = QueryParser::for_index(index, default_fields);
    let query = match parser.parse_query(&info.q) {
        Ok(q) => q,
        Err(e) => return HttpResponse::BadRequest().body(format!("invalid query: {}", e)),
    };
    let limit = info.limit.unwrap_or(10);
    let top_docs = match searcher.search(&query, &TopDocs::with_limit(limit)) {
        Ok(d) => d,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    let mut results: Vec<serde_json::Value> = Vec::new();
    for (_score, addr) in top_docs {
        let doc: TantivyDocument = match searcher.doc::<TantivyDocument>(addr) {
            Ok(d) => d,
            Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
        };
        results.push(doc_to_named_debug(&schema, &doc));
    }

    HttpResponse::Ok().json(results)
}

#[post("/update")]
async fn update_document(data: web::Json<BlogPost>, state: web::Data<AppState>) -> impl Responder {
    let mut writer = match state.writer.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    };
    let schema = writer.index().schema();
    let f_id = schema.get_field("id").unwrap();

    // delete existing by id, then add
    writer.delete_term(Term::from_field_text(f_id, &data.id));
    match index_post(&mut writer, &schema, data.into_inner()) {
        Ok(_) => HttpResponse::Ok().json("updated"),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

#[derive(Deserialize)]
struct DeleteQuery { id: String }

#[delete("/delete")]
async fn delete_document(info: web::Query<DeleteQuery>, state: web::Data<AppState>) -> impl Responder {
    let mut writer = match state.writer.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    };
    let schema = writer.index().schema();
    let f_id = schema.get_field("id").unwrap();
    writer.delete_term(Term::from_field_text(f_id, &info.id));
    HttpResponse::Ok().json("deleted")
}

fn doc_to_named_debug(schema: &Schema, doc: &TantivyDocument) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for fv in doc.field_values() {
        let name = schema.get_field_entry(fv.field()).name().to_string();
        obj.insert(name, serde_json::Value::String(format!("{:?}", fv.value())));
    }
    serde_json::Value::Object(obj)
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Build schema and index in a temp dir (RAM directory is also possible). Use project-local path.
    let schema = create_schema();

    let mut index_path = PathBuf::from(".tantivy_idx");
    // Create or open index
    let index = if index_path.exists() {
        Index::open_in_dir(&index_path)?
    } else {
        std::fs::create_dir_all(&index_path)?;
        Index::create_in_dir(&index_path, schema.clone())?
    };

    // Register custom analyzers (no external deps):
    // - zh_ngram: character bigram/trigram for CJK-friendly search
    // - whitespace_lc: whitespace + lowercasing for tags
    {
        let zh = TextAnalyzer::builder(NgramTokenizer::new(2, 3, false).unwrap())
            .filter(LowerCaser)
            .build();
        index.tokenizers().register("zh_ngram", zh);

        let tags_analyzer = TextAnalyzer::builder(WhitespaceTokenizer::default())
            .filter(LowerCaser)
            .build();
        index.tokenizers().register("whitespace_lc", tags_analyzer);
    }

    // Create writer with 50MB heap
    let writer = index.writer(50_000_000)?;
    let reader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into()?;
    let searcher = reader.searcher();

    let state = web::Data::new(AppState {
        writer: Arc::new(Mutex::new(writer)),
        reader,
        current_searcher: Arc::new(ArcSwap::new(Arc::new(searcher))),
    });

    // Background task to periodically commit and refresh searcher
    {
        let state_clone = state.clone();
        actix_web::rt::spawn(async move {
            loop {
                actix_web::rt::time::sleep(Duration::from_secs(3)).await;
                // commit
                if let Ok(mut w) = state_clone.writer.lock() {
                    if let Err(e) = w.commit() {
                        eprintln!("commit error: {}", e);
                        continue;
                    }
                }
                // reload reader and swap searcher
                if let Err(e) = state_clone.reader.reload() {
                    eprintln!("reader reload error: {}", e);
                    continue;
                }
                let new_searcher = state_clone.reader.searcher();
                state_clone
                    .current_searcher
                    .store(Arc::new(new_searcher));
            }
        });
    }

    println!("Server running at http://127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(add_document)
            .service(update_document)
            .service(delete_document)
            .service(search_document)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    Ok(())
}
