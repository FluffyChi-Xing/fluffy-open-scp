use crate::{background::write_atomic, session::Session};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::Incoming,
    header::{CACHE_CONTROL, CONTENT_TYPE, HOST},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use std::{
    convert::Infallible,
    io,
    net::{Ipv4Addr, SocketAddrV4},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, sync::Semaphore};

type Reply = Response<Full<Bytes>>;

fn response(status: StatusCode, data: Bytes) -> Reply {
    let mut reply = Response::new(Full::new(data));
    *reply.status_mut() = status;
    if status == StatusCode::OK {
        reply.headers_mut().insert(
            CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("image/png"),
        );
        reply.headers_mut().insert(
            CACHE_CONTROL,
            hyper::header::HeaderValue::from_static("no-store"),
        );
    }
    reply
}

async fn route(req: Request<Incoming>, session: Arc<Mutex<Session>>, host: Arc<str>) -> Reply {
    if req.method() != Method::GET
        || req.headers().get(HOST).and_then(|h| h.to_str().ok()) != Some(host.as_ref())
        || req.uri().query().is_some()
        || req.uri().authority().is_some()
    {
        return response(StatusCode::NOT_FOUND, Bytes::new());
    }
    let path = req.uri().path().to_owned();
    match tokio::task::spawn_blocking(move || {
        let mut session = session
            .lock()
            .map_err(|_| io::Error::other("Preview state unavailable"))?;
        session.handle(&path)
    })
    .await
    {
        Ok(Ok(Some(data))) => response(StatusCode::OK, data),
        Ok(Ok(None)) => response(StatusCode::NOT_FOUND, Bytes::new()),
        _ => response(StatusCode::INTERNAL_SERVER_ERROR, Bytes::new()),
    }
}

/// Serve the legacy updater protocol on IPv4 loopback. Port 0 selects a free port.
pub async fn serve(directory: &Path, port: u16) -> io::Result<()> {
    let session = Arc::new(Mutex::new(Session::open(directory)?));
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).await?;
    let address = listener.local_addr()?;
    let host: Arc<str> = address.to_string().into();
    write_atomic(
        &directory.join("ready.json"),
        &serde_json::to_vec(
            &serde_json::json!({"pid": std::process::id(), "port": address.port(), "backend": "rust"}),
        )?,
    )?;
    let connections = Arc::new(Semaphore::new(16));
    loop {
        let permit = Arc::clone(&connections)
            .acquire_owned()
            .await
            .map_err(io::Error::other)?;
        let (stream, _) = listener.accept().await?;
        let session = Arc::clone(&session);
        let host = Arc::clone(&host);
        tokio::spawn(async move {
            let _permit = permit;
            let service = service_fn(move |req| {
                let session = Arc::clone(&session);
                let host = Arc::clone(&host);
                async { Ok::<_, Infallible>(route(req, session, host).await) }
            });
            let conn = http1::Builder::new()
                .keep_alive(false)
                .serve_connection(TokioIo::new(stream), service);
            let _ = tokio::time::timeout(Duration::from_secs(10), conn).await;
        });
    }
}
