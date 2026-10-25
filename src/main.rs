mod db;
mod runtime;
mod server;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Parser)]
#[command(
    name = "yamanami",
    version,
    about = "Experimental WebAssembly application server"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve(server::Options),
    /// Operate a running server through its management API.
    Ctl(Control),
}
#[derive(Args)]
struct Control {
    #[arg(long, default_value = "http://127.0.0.1:9090")]
    endpoint: String,
    #[arg(long, env = "YAMANAMI_TOKEN", hide_env_values = true)]
    token: String,
    #[command(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    Deploy {
        wasm: PathBuf,
        #[arg(long)]
        app: String,
        #[arg(long)]
        path: String,
    },
    List,
    Info {
        app: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Serve(options) => server::serve(options).await,
        Command::Ctl(control) => control.run().await,
    }
}
impl Control {
    async fn run(self) -> Result<()> {
        // The first-stage management protocol is strictly local HTTP.
        let url = reqwest::Url::parse(&self.endpoint)?;
        let host = url.host_str().context("endpoint has no host")?;
        let ip = host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .context("use a loopback IP address for --endpoint")?;
        let address = SocketAddr::new(
            ip,
            url.port_or_known_default()
                .context("endpoint has no port")?,
        );
        if url.scheme() != "http"
            || !address.ip().is_loopback()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            bail!("management endpoint must be a loopback HTTP origin");
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(120))
            .build()?;
        let origin = self.endpoint.trim_end_matches('/');
        let request = match self.action {
            Action::Deploy { wasm, app, path } => {
                server::validate_app(&app)?;
                server::validate_path(&path)?;
                let size = tokio::fs::metadata(&wasm).await?.len();
                if size > server::MAX_UPLOAD as u64 {
                    bail!("Wasm exceeds the {} byte upload limit", server::MAX_UPLOAD);
                }
                let body = tokio::fs::read(&wasm).await?;
                client
                    .put(format!("{origin}/v1/apps/{app}"))
                    .header("x-yamanami-path", path)
                    .header("content-type", "application/wasm")
                    .body(body)
            }
            Action::List => client.get(format!("{origin}/v1/apps")),
            Action::Info { app } => {
                server::validate_app(&app)?;
                client.get(format!("{origin}/v1/apps/{app}"))
            }
        };
        let response = request.bearer_auth(self.token).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("management API returned {status}: {body}");
        }
        let value: serde_json::Value = serde_json::from_str(&body)?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        Ok(())
    }
}
