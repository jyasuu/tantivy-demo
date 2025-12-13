use anyhow::Result;
use clap::Parser;
use reqwest::Client;
use serde_json::Value;

#[derive(Parser, Debug, Clone)]
#[command(name = "search", about = "Query the search service")] 
pub struct Opts {
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub endpoint: String,

    #[arg(long, default_value = "rust")] 
    pub q: String,

    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let client = Client::builder().build()?;

    let url = format!("{}/search", opts.endpoint);
    let resp = client.get(url).query(&[("q", &opts.q), ("limit", &opts.limit.to_string())]).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("search failed: {} - {}", status, text);
    }
    let json: Value = resp.json().await?;
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}
