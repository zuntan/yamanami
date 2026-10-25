use anyhow::{Context, Result, bail};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{Request, Response};
use std::{sync::Arc, time::Duration};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::p2::bindings::{ProxyPre, http::types::Scheme};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpCtxView, WasiHttpHooks, WasiHttpView};

pub type Prepared = Arc<ProxyPre<Guest>>;
pub struct Guest {
    table: ResourceTable,
    wasi: WasiCtx,
    http: WasiHttpCtx,
    limits: StoreLimits,
    hooks: DenyNetwork,
}
impl WasiView for Guest {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}
impl WasiHttpView for Guest {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: &mut self.hooks,
        }
    }
}
struct DenyNetwork;
impl WasiHttpHooks for DenyNetwork {
    fn send_request(
        &mut self,
        _: Request<wasmtime_wasi_http::WasiBody>,
        _: Option<wasmtime_wasi_http::RequestOptions>,
        _: Box<dyn Future<Output = wasmtime_wasi_http::Result<()>> + Send>,
    ) -> Box<
        dyn Future<
                Output = wasmtime_wasi_http::Result<(
                    Response<wasmtime_wasi_http::WasiBody>,
                    Box<dyn Future<Output = wasmtime_wasi_http::Result<()>> + Send>,
                )>,
            > + Send,
    > {
        Box::new(async { Err(wasmtime_wasi_http::Error::HttpRequestDenied) })
    }
    fn p2_outgoing_body_chunk_size(&mut self) -> usize {
        16 * 1024
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub engine: Engine,
    pub memory: usize,
    pub fuel: u64,
    pub timeout: Duration,
    pub body_limit: usize,
    pub max_memories: usize,
    pub max_instances: usize,
    pub max_tables: usize,
}
impl Runtime {
    pub fn new(
        memory: usize,
        fuel: u64,
        timeout: Duration,
        body_limit: usize,
        opt_level: wasmtime::OptLevel,
    ) -> Result<Self> {
        let mut config = Config::new();
        config
            .cranelift_opt_level(opt_level)
            .wasm_component_model(true)
            .consume_fuel(true)
            .epoch_interruption(true);
        let engine = Engine::new(&config)?;
        Ok(Self {
            engine,
            memory,
            fuel,
            timeout,
            body_limit,
            max_memories: 1,
            max_instances: 32,
            max_tables: 8,
        })
    }

    pub async fn compile(&self, wasm: Vec<u8>) -> Result<Prepared> {
        let engine = self.engine.clone();
        tokio::task::spawn_blocking(move || {
            let component = Component::new(&engine, &wasm)
                .map_err(anyhow::Error::from)
                .context("compile Wasm component")?;
            let mut linker = Linker::new(&engine);
            wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
            wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;
            Ok(Arc::new(ProxyPre::new(
                linker.instantiate_pre(&component)?,
            )?))
        })
        .await?
    }

    pub async fn execute(
        &self,
        pre: Prepared,
        req: Request<Full<Bytes>>,
    ) -> Result<Response<Full<Bytes>>> {
        let mut wasi = WasiCtx::builder();
        wasi.allow_tcp(false)
            .allow_udp(false)
            .allow_ip_name_lookup(false);
        let mut http = WasiHttpCtx::new();
        http.set_field_size_limit(32 * 1024);
        let mut store = Store::new(
            &self.engine,
            Guest {
                table: ResourceTable::new(),
                wasi: wasi.build(),
                http,
                limits: StoreLimitsBuilder::new()
                    .memory_size(self.memory)
                    .table_elements(10000)
                    .instances(self.max_instances)
                    .memories(self.max_memories)
                    .tables(self.max_tables)
                    .trap_on_grow_failure(true)
                    .build(),
                hooks: DenyNetwork,
            },
        );
        store.limiter(|guest| &mut guest.limits);
        store.set_fuel(self.fuel)?;
        store.fuel_async_yield_interval(Some(10000))?;
        // An independent epoch clock also interrupts CPU loops. The outer timeout covers I/O.
        store.set_epoch_deadline((self.timeout.as_millis() / 10 + 1) as u64);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let req = store
            .data_mut()
            .http()
            .new_incoming_request(Scheme::Http, req)?;
        let out = store.data_mut().http().new_response_outparam(sender)?;
        // Poll execution and response collection together: a guest may block on writing its body.
        // Neither future is detached, so timeout or cancellation drops the entire Store.
        let execution = async move {
            let proxy = pre.instantiate_async(&mut store).await?;
            proxy
                .wasi_http_incoming_handler()
                .call_handle(&mut store, req, out)
                .await?;
            Ok::<_, anyhow::Error>(())
        };
        let limit = self.body_limit;
        let response = async move {
            let response = receiver
                .await
                .context("guest did not produce a response")?
                .map_err(|e| anyhow::anyhow!("guest response error: {e:?}"))?;
            if response.status().is_informational() {
                bail!("informational responses are unsupported");
            }
            let (mut parts, body) = response.into_parts();
            let body = Limited::new(body, limit)
                .collect()
                .await
                .map_err(anyhow::Error::from_boxed)
                .context("guest response body")?
                .to_bytes();
            // Hyper frames the buffered response; do not trust guest framing.
            parts.headers.remove(hyper::header::TRANSFER_ENCODING);
            parts.headers.remove(hyper::header::CONTENT_LENGTH);
            Ok::<_, anyhow::Error>(Response::from_parts(parts, Full::new(body)))
        };
        let (_, response) = tokio::time::timeout(self.timeout, async {
            tokio::try_join!(execution, response)
        })
        .await
        .context("guest deadline exceeded")??;
        Ok(response)
    }
}
