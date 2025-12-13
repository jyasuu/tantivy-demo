use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Parser;
use rand::distributions::{Alphanumeric, DistString};
use rand::{seq::SliceRandom, Rng};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::Semaphore;

#[derive(Parser, Debug, Clone)]
#[command(name = "generate", about = "Generate and index a large number of documents")] 
pub struct Opts {
    #[arg(long, default_value_t = 1000)]
    pub count: usize,

    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,

    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub endpoint: String,
}

#[derive(Serialize, Debug, Clone)]
struct BlogPost {
    id: String,
    title: String,
    body: String,
    tags: Vec<String>,
    create_at: Option<i64>,
    status: String,
    features: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let client = Client::builder().build()?;

    let semaphore = Arc::new(Semaphore::new(opts.concurrency));
    let mut handles = Vec::with_capacity(opts.count);

    let tags_pool = vec!["rust", "search", "tantivy", "actix", "json", "indexing", "performance", "concurrency"];    

    for i in 0..opts.count {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let endpoint = opts.endpoint.clone();
        let tags_pool = tags_pool.clone();

        let handle = tokio::spawn(async move {
            let _p = permit;
            let id = format!("doc-{}-{}", i, rand::thread_rng().gen::<u64>());
            let title = format!("Post {} about Rust and search", i);
            let body = random_body(200 + (i % 200) as usize);
            let tags = random_tags(&tags_pool, 1 + (i % 4) as usize);
            let create_at = Some(now_secs() as i64);
            let status = if i % 5 == 0 { "draft" } else { "published" }.to_string();
            let lang = ["en", "zh", "jp", "fr"].choose(&mut rand::thread_rng()).unwrap().to_string();
            let random = Alphanumeric.sample_string(&mut rand::thread_rng(), 12);
            let features = serde_json::json!({
                "lang": lang,
                "length": body.len(),
                "score": (i as f64) * 0.1,
                "random": random,
            });

            let post = BlogPost { id, title, body, tags, create_at, status, features };
            let url = format!("{}/index", endpoint);
            let resp = client.post(&url).json(&post).send().await.context("request failed")?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("index failed: {} - {}", status, text);
            }
            Ok::<_, anyhow::Error>(())
        });
        handles.push(handle);
    }

    // Wait for all
    let mut ok = 0usize;
    for h in handles {
        match h.await? {
            Ok(()) => ok += 1,
            Err(e) => eprintln!("index error: {}", e),
        }
    }
    println!("Indexed {}/{} documents", ok, opts.count);
    Ok(())
}

fn random_body(len: usize) -> String {
    // Generate random lorem-like content
    let words = ["rust", "search", "engine", "tantivy", "fast", "index", "query", "http", "json", "analysis", "token", "field", "document", "commit", "reload", "reader", "writer", "arc", "mutex", "swap"];    
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| words.choose(&mut rng).unwrap().to_string())
        .collect::<Vec<String>>()
        .join(" ")
}

fn random_tags(pool: &[&str], n: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut tags: Vec<String> = pool
        .choose_multiple(&mut rng, n.min(pool.len()))
        .cloned()
        .map(|s| s.to_string())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
