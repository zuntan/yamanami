use crate::{
    db::{App, Database},
    runtime::{Prepared, Runtime},
};
use anyhow::{Context, Result, bail, ensure};
use bytes::Bytes;
use clap::Args;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{Request, Response, StatusCode, body::Incoming};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::{
    net::TcpListener,
    sync::{RwLock, Semaphore},
    task::JoinSet,
};

pub const MAX_UPLOAD: usize = 16 * 1024 * 1024;
const MAX_APPS: usize = 32;

#[derive(Args, Clone)]
pub struct Options {
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: SocketAddr,
    #[arg(long, default_value = "127.0.0.1:9090")]
    admin_listen: SocketAddr,
    #[arg(long, default_value = "var/yamanami.sqlite3")]
    database: PathBuf,
    #[arg(long, env = "YAMANAMI_TOKEN", hide_env_values = true)]
    token: String,
    #[arg(long, default_value_t = 64)]
    memory_mib: usize,
    #[arg(long, default_value_t = 10_000_000)]
    fuel: u64,
    #[arg(long, default_value_t = 3000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 16)]
    concurrency: usize,
    #[arg(long, default_value_t = 4)]
    app_concurrency: usize,
    #[arg(long, default_value_t = 1024 * 1024)]
    body_limit: usize,
}

#[derive(Clone)]
struct Active {
    app: App,
    pre: Prepared,
    permits: Arc<Semaphore>,
}
struct State {
    db: Database,
    runtime: Runtime,
    apps: RwLock<BTreeMap<String, Active>>,
    // Deployment operations are serialized and continue through commit/publication if the client leaves.
    deploy: Arc<Semaphore>,
    cache: tokio::sync::Mutex<VecDeque<(String, Prepared)>>,
    requests: Semaphore,
    token_hash: [u8; 32],
    app_concurrency: usize,
}
type Reply = Response<Full<Bytes>>;
type Failure = (StatusCode, String);
type ApiResult = std::result::Result<Reply, Failure>;

fn fail(status: StatusCode, message: impl ToString) -> Failure {
    (status, message.to_string())
}
fn internal(error: impl std::fmt::Display) -> Failure {
    eprintln!("internal error: {error}");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}
fn json(status: StatusCode, value: &impl Serialize) -> Reply {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(
            serde_json::to_vec(value).expect("serializable API response"),
        )))
        .unwrap()
}
fn error_reply(error: Failure) -> Reply {
    let mut response = json(error.0, &serde_json::json!({"error": error.1}));
    // An error may occur before the incoming body is consumed.
    response.headers_mut().insert(
        hyper::header::CONNECTION,
        hyper::header::HeaderValue::from_static("close"),
    );
    response
}

pub fn validate_app(app: &str) -> Result<()> {
    ensure!(
        !app.is_empty()
            && app.len() <= 64
            && app
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "app must be 1..64 ASCII letters, digits, '-' or '_'"
    );
    Ok(())
}
pub fn validate_path(path: &str) -> Result<()> {
    ensure!(
        path.starts_with('/')
            && path.len() <= 256
            && (path == "/" || !path.ends_with('/'))
            && path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
            && !path.split('/').any(|s| s == "." || s == "..")
            && !path.contains("//"),
        "path must be an absolute ASCII path, without a trailing slash, empty or dot segments"
    );
    Ok(())
}
fn matches(prefix: &str, path: &str) -> bool {
    prefix == "/"
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|s| s.starts_with('/'))
}

pub async fn serve(options: Options) -> Result<()> {
    ensure!(
        options.listen.ip().is_loopback(),
        "first-stage public listener must use a loopback address"
    );
    ensure!(
        options.admin_listen.ip().is_loopback(),
        "management listener must use a loopback address"
    );
    ensure!(
        options.listen != options.admin_listen,
        "HTTP and management listeners must differ"
    );
    ensure!(
        options.token.len() >= 16
            && options.token.is_ascii()
            && !options.token.bytes().any(|b| b.is_ascii_control()),
        "management token must contain at least 16 printable ASCII bytes"
    );
    ensure!(
        (1..=1024).contains(&options.memory_mib),
        "memory-mib must be 1..1024"
    );
    ensure!(
        options.fuel > 0 && (1..=60000).contains(&options.timeout_ms),
        "fuel must be positive; timeout-ms must be 1..60000"
    );
    ensure!(
        (1..=256).contains(&options.concurrency) && (1..=256).contains(&options.app_concurrency),
        "concurrency must be 1..256"
    );
    ensure!(
        (1..=16 * 1024 * 1024).contains(&options.body_limit),
        "body-limit must be 1..16777216"
    );
    let runtime = Runtime::new(
        options.memory_mib * 1024 * 1024,
        options.fuel,
        Duration::from_millis(options.timeout_ms),
        options.body_limit,
    )?;
    let db_path = options.database.clone();
    let (db, _database_lock) = tokio::task::spawn_blocking(move || -> Result<_> {
        if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut lock_path = db_path.as_os_str().to_owned();
        lock_path.push(".lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.try_lock()
            .context("database is already in use by another server")?;
        Ok((Database::open(&db_path)?, lock))
    })
    .await??;
    let state = Arc::new(State {
        db,
        runtime,
        apps: RwLock::new(BTreeMap::new()),
        deploy: Arc::new(Semaphore::new(1)),
        cache: tokio::sync::Mutex::new(VecDeque::new()),
        requests: Semaphore::new(options.concurrency),
        token_hash: Sha256::digest(options.token.as_bytes()).into(),
        app_concurrency: options.app_concurrency,
    });
    let active = state.db.active().await?;
    ensure!(
        active.len() <= MAX_APPS,
        "too many persisted applications (limit {MAX_APPS})"
    );
    for (app, wasm) in active {
        validate_app(&app.name)?;
        validate_path(&app.path)?;
        ensure!(
            format!("{:x}", Sha256::digest(&wasm)) == app.sha256,
            "stored Wasm hash mismatch"
        );
        let pre = state.prepare(app.sha256.clone(), wasm).await?;
        state.apps.write().await.insert(
            app.name.clone(),
            Active {
                app,
                pre,
                permits: Arc::new(Semaphore::new(options.app_concurrency)),
            },
        );
    }
    let public = TcpListener::bind(options.listen)
        .await
        .context("bind public HTTP listener")?;
    let admin = TcpListener::bind(options.admin_listen)
        .await
        .context("bind management listener")?;
    println!("HTTP http://{}", public.local_addr()?);
    println!("Management http://{}", admin.local_addr()?);
    let mut tasks = JoinSet::new();
    let engine = state.runtime.engine.clone();
    tasks.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        loop {
            interval.tick().await;
            engine.increment_epoch();
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    tasks.spawn(listen(public, state.clone(), false));
    tasks.spawn(listen(admin, state, true));
    tokio::select! {
        result = tasks.join_next() => { bail!("server task exited: {result:?}"); }
        result = tokio::signal::ctrl_c() => { result?; }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}

async fn listen(listener: TcpListener, state: Arc<State>, admin: bool) -> Result<()> {
    let connections = Arc::new(Semaphore::new(if admin { 8 } else { 128 }));
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (socket, _) = accepted?;
                let Ok(permit) = connections.clone().try_acquire_owned() else { continue };
                let state = state.clone();
                tasks.spawn(async move {
                    let _permit = permit;
                    let service = hyper::service::service_fn(move |req| {
                        let state = state.clone();
                        async move {
                            let result = if admin { state.manage(req).await } else { state.request(req).await };
                            Ok::<_, Infallible>(result.unwrap_or_else(error_reply))
                        }
                    });
                    let mut builder = hyper::server::conn::http1::Builder::new();
                    builder.keep_alive(false).max_headers(64).max_buf_size(32 * 1024);
                    let connection = builder.serve_connection(TokioIo::new(socket), service);
                    // Also bounds slow headers, upload bodies and clients that stop reading.
                    let _ = tokio::time::timeout(Duration::from_secs(if admin { 120 } else { 65 }), connection).await;
                });
            }
            _ = tasks.join_next(), if !tasks.is_empty() => {}
        }
    }
}
impl State {
    async fn prepare(&self, hash: String, wasm: Vec<u8>) -> Result<Prepared> {
        // Called only during startup or while holding the deployment permit.
        let mut cache = self.cache.lock().await;
        if let Some(index) = cache.iter().position(|(key, _)| *key == hash) {
            let entry = cache.remove(index).unwrap();
            let pre = entry.1.clone();
            cache.push_back(entry);
            return Ok(pre);
        }
        let pre = self.runtime.compile(wasm).await?;
        if cache.len() >= 8 {
            cache.pop_front();
        }
        cache.push_back((hash, pre.clone()));
        Ok(pre)
    }
    async fn request(&self, req: Request<Incoming>) -> ApiResult {
        let _global = self
            .requests
            .try_acquire()
            .map_err(|_| fail(StatusCode::SERVICE_UNAVAILABLE, "server concurrency limit"))?;
        let active = self
            .apps
            .read()
            .await
            .values()
            .filter(|entry| matches(&entry.app.path, req.uri().path()))
            .max_by_key(|entry| entry.app.path.len())
            .cloned()
            .ok_or_else(|| fail(StatusCode::NOT_FOUND, "application not found"))?;
        let _local = active.permits.try_acquire().map_err(|_| {
            fail(
                StatusCode::SERVICE_UNAVAILABLE,
                "application concurrency limit",
            )
        })?;
        let (parts, body) = req.into_parts();
        let bytes = tokio::time::timeout(
            Duration::from_secs(10),
            Limited::new(body, self.runtime.body_limit).collect(),
        )
        .await
        .map_err(|_| {
            fail(
                StatusCode::REQUEST_TIMEOUT,
                "request body deadline exceeded",
            )
        })?
        .map_err(|_| {
            fail(
                StatusCode::PAYLOAD_TOO_LARGE,
                "invalid or oversized request body",
            )
        })?
        .to_bytes();
        match self
            .runtime
            .execute(active.pre, Request::from_parts(parts, Full::new(bytes)))
            .await
        {
            Ok(response) => Ok(response),
            Err(error) => {
                eprintln!(
                    "guest {} release {}: {error:#}",
                    active.app.name, active.app.release
                );
                Err(fail(
                    StatusCode::BAD_GATEWAY,
                    "Wasm execution failed or exceeded its limits",
                ))
            }
        }
    }

    async fn manage(self: Arc<Self>, req: Request<Incoming>) -> ApiResult {
        let auth = req
            .headers()
            .get(hyper::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("");
        let digest: [u8; 32] = Sha256::digest(auth.as_bytes()).into();
        if !bool::from(digest.ct_eq(&self.token_hash)) {
            return Err(fail(StatusCode::UNAUTHORIZED, "invalid management token"));
        }
        let path = req.uri().path();
        if req.method() == hyper::Method::GET && path == "/v1/apps" {
            let apps: Vec<_> = self
                .apps
                .read()
                .await
                .values()
                .map(|a| a.app.clone())
                .collect();
            return Ok(json(StatusCode::OK, &apps));
        }
        let name = path
            .strip_prefix("/v1/apps/")
            .ok_or_else(|| fail(StatusCode::NOT_FOUND, "management endpoint not found"))?
            .to_string();
        validate_app(&name).map_err(|e| fail(StatusCode::BAD_REQUEST, e))?;
        if req.method() == hyper::Method::GET {
            let apps = self.apps.read().await;
            return apps
                .get(&name)
                .map(|a| json(StatusCode::OK, &a.app))
                .ok_or_else(|| fail(StatusCode::NOT_FOUND, "application not found"));
        }
        if req.method() != hyper::Method::PUT {
            return Err(fail(StatusCode::METHOD_NOT_ALLOWED, "use GET or PUT"));
        }
        let permit = self.deploy.clone().try_acquire_owned().map_err(|_| {
            fail(
                StatusCode::SERVICE_UNAVAILABLE,
                "another deployment is running",
            )
        })?;
        let mount = req
            .headers()
            .get("x-yamanami-path")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| fail(StatusCode::BAD_REQUEST, "missing x-yamanami-path"))?
            .to_string();
        validate_path(&mount).map_err(|e| fail(StatusCode::BAD_REQUEST, e))?;
        {
            let apps = self.apps.read().await;
            if !apps.contains_key(&name) && apps.len() >= MAX_APPS {
                return Err(fail(StatusCode::CONFLICT, "application limit reached"));
            }
        }
        if self
            .db
            .path_owner(mount.clone())
            .await
            .map_err(internal)?
            .is_some_and(|owner| owner != name)
        {
            return Err(fail(StatusCode::CONFLICT, "path is already assigned"));
        }
        let wasm = tokio::time::timeout(
            Duration::from_secs(30),
            Limited::new(req.into_body(), MAX_UPLOAD).collect(),
        )
        .await
        .map_err(|_| fail(StatusCode::REQUEST_TIMEOUT, "upload deadline exceeded"))?
        .map_err(|_| fail(StatusCode::PAYLOAD_TOO_LARGE, "invalid or oversized upload"))?
        .to_bytes()
        .to_vec();
        // Once validation starts, finish the atomic database/published-route update even if
        // the caller disconnects. The permit bounds these detached management operations.
        tokio::spawn(async move {
            let _permit = permit;
            let hash = format!("{:x}", Sha256::digest(&wasm));
            let pre = self
                .prepare(hash.clone(), wasm.clone())
                .await
                .map_err(|e| {
                    fail(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        format!("invalid HTTP component: {e:#}"),
                    )
                })?;
            let mut apps = self.apps.write().await;
            let app = self
                .db
                .deploy(name.clone(), mount, hash, wasm)
                .await
                .map_err(internal)?;
            let permits = apps
                .get(&name)
                .map(|a| a.permits.clone())
                .unwrap_or_else(|| Arc::new(Semaphore::new(self.app_concurrency)));
            apps.insert(
                name,
                Active {
                    app: app.clone(),
                    pre,
                    permits,
                },
            );
            Ok(json(StatusCode::CREATED, &app))
        })
        .await
        .map_err(internal)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_boundaries_and_validation() {
        assert!(matches("/hello", "/hello"));
        assert!(matches("/hello", "/hello/world"));
        assert!(!matches("/hello", "/helloworld"));
        assert!(matches("/", "/anywhere"));
        for path in ["/", "/hello", "/hello-v2/api"] {
            assert!(validate_path(path).is_ok());
        }
        for path in ["", "hello", "/a/", "/a//b", "/a/../b", "/a?b", "/a%2Fb"] {
            assert!(validate_path(path).is_err(), "{path}");
        }
        assert!(validate_app("shop_2").is_ok());
        assert!(validate_app("../shop").is_err());
    }
}
