use clap::Parser;
use reqwest::Url;
use std::{error::Error, path::PathBuf};
use tokio_util::io::ReaderStream;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    file_path: PathBuf,

    #[arg(short, long)]
    server: Option<String>,

    #[arg(short, long)]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let server_env = std::env::var("CANNON_SERVER");
    let api_key_env = std::env::var("CANNON_API_KEY");

    // TODO: this is inverted, the command line args should take precendence over the env vars
    let server: String = match server_env {
        Ok(server) => server,
        Err(_) => args
            .server
            .expect("Specify CANNON_SERVER as an env var or pass it in as an argument."),
    };

    let api_key = match api_key_env {
        Ok(api_key) => api_key,
        Err(_) => args
            .api_key
            .expect("Specify CANNON_API_KEY as an env var or pass it in as an argument"),
    };

    let filename = args
        .file_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("File path has no valid filename")?
        .to_owned();

    let file = tokio::fs::File::open(&filename).await?;
    let body = reqwest::Body::wrap_stream(ReaderStream::new(file));

    let response = reqwest::Client::new()
        .put(&server)
        .bearer_auth(api_key)
        .query(&[("filename", filename)])
        .header("Content-Type", "application/octet-stream")
        .body(body)
        .send()
        .await?;

    let status = response.status();
    let response_body = response.text().await?;

    if !status.is_success() {
        return Err(format!("Upload failed ({status}): {response_body}").into());
    }

    let url = Url::parse(server.as_str())?.join(&response_body)?;
    println!("{url}");
    Ok(())
}
