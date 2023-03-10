use axum::{
    body::{Bytes, Full},
    error_handling::HandleErrorLayer,
    extract::{rejection::LengthLimitError, DefaultBodyLimit},
    http::{self, StatusCode},
    middleware, routing, BoxError, Router, Server,
};
use futures::future;
use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::LazyLock, time::Duration};
use tokio::runtime::Builder;
use tower::{
    timeout::{error::Elapsed, TimeoutLayer},
    ServiceBuilder,
};
use tower_cookies::CookieManagerLayer;
use tower_http::{
    add_extension::AddExtensionLayer,
    compression::{
        predicate::{DefaultPredicate, NotForContentType, Predicate},
        CompressionLayer,
    },
    decompression::DecompressionLayer,
    services::{ServeDir, ServeFile},
};
use zino_core::{
    application::Application,
    extend::TomlTableExt,
    response::Response,
    schedule::{AsyncCronJob, Job, JobScheduler},
    state::State,
};

/// An HTTP server cluster for `axum`.
#[derive(Default)]
pub struct AxumCluster {
    /// Routes.
    routes: Vec<Router>,
}

impl Application for AxumCluster {
    /// Router.
    type Router = Router;

    /// Registers routes.
    fn register(mut self, routes: Vec<Self::Router>) -> Self {
        self.routes = routes;
        self
    }

    /// Runs the application.
    fn run(self, async_jobs: Vec<(&'static str, AsyncCronJob)>) {
        let runtime = Builder::new_multi_thread()
            .thread_keep_alive(Duration::from_secs(10))
            .thread_stack_size(2 * 1024 * 1024)
            .global_queue_interval(61)
            .enable_all()
            .build()
            .expect("fail to build Tokio runtime with the multi thread scheduler selected");
        let mut scheduler = JobScheduler::new();
        for (cron_expr, exec) in async_jobs {
            scheduler.add(Job::new_async(cron_expr, exec));
        }
        runtime.spawn(async move {
            loop {
                scheduler.tick_async().await;

                // Cannot use `std::thread::sleep` because it blocks the Tokio runtime.
                tokio::time::sleep(scheduler.time_till_next_job()).await;
            }
        });

        // Server config.
        let mut body_limit = 100 * 1024 * 1024; // 100MB
        let mut request_timeout = Duration::from_secs(10); // 10 seconds
        let mut public_dir = PathBuf::new();
        let default_public_dir = Self::project_dir().join("assets");
        if let Some(server) = Self::config().get_table("server") {
            if let Some(limit) = server.get_usize("body-limit") {
                body_limit = limit;
            }
            if let Some(timeout) = server.get_duration("request-timeout") {
                request_timeout = timeout;
            }
            if let Some(dir) = server.get_str("public-dir") {
                public_dir.push(dir);
            } else {
                public_dir = default_public_dir;
            }
        } else {
            public_dir = default_public_dir;
        }
        let index_file = public_dir.join("index.html");
        let not_found_file = public_dir.join("404.html");
        let serve_file = ServeFile::new(index_file);
        let serve_dir = ServeDir::new(public_dir)
            .precompressed_gzip()
            .precompressed_br()
            .not_found_service(ServeFile::new(not_found_file));

        runtime.block_on(async {
            let routes = self.routes;
            let app_state = State::default();
            let app_env = app_state.env();
            let listeners = app_state.listeners();
            let servers = listeners.iter().map(|listener| {
                let mut app = Router::new()
                    .route_service("/", serve_file.clone())
                    .nest_service("/assets", serve_dir.clone())
                    .route("/sse", routing::get(crate::endpoint::axum_sse::sse_handler))
                    .route(
                        "/websocket",
                        routing::get(crate::endpoint::axum_websocket::websocket_handler),
                    );
                for route in &routes {
                    app = app.merge(route.clone());
                }

                let state = app_state.clone();
                app = app
                    .fallback_service(tower::service_fn(|req| async {
                        let request = crate::AxumExtractor(req);
                        let res = Response::new(StatusCode::NOT_FOUND).provide_context(&request);
                        Ok::<http::Response<Full<Bytes>>, Infallible>(res.into())
                    }))
                    .layer(
                        ServiceBuilder::new()
                            .layer(AddExtensionLayer::new(state))
                            .layer(DefaultBodyLimit::max(body_limit))
                            .layer(CookieManagerLayer::new())
                            .layer(
                                CompressionLayer::new().gzip(true).br(true).compress_when(
                                    DefaultPredicate::new()
                                        .and(NotForContentType::new("application/msgpack")),
                                ),
                            )
                            .layer(DecompressionLayer::new().gzip(true).br(true))
                            .layer(LazyLock::force(
                                &crate::middleware::tower_tracing::TRACING_MIDDLEWARE,
                            ))
                            .layer(LazyLock::force(
                                &crate::middleware::tower_cors::CORS_MIDDLEWARE,
                            ))
                            .layer(middleware::from_fn(
                                crate::middleware::axum_context::request_context,
                            ))
                            .layer(HandleErrorLayer::new(|err: BoxError| async move {
                                let status_code = if err.is::<Elapsed>() {
                                    StatusCode::REQUEST_TIMEOUT
                                } else if err.is::<LengthLimitError>() {
                                    StatusCode::PAYLOAD_TOO_LARGE
                                } else {
                                    StatusCode::INTERNAL_SERVER_ERROR
                                };
                                let res = Response::new(status_code);
                                Ok::<http::Response<Full<Bytes>>, Infallible>(res.into())
                            }))
                            .layer(TimeoutLayer::new(request_timeout)),
                    );
                tracing::warn!(env = app_env, "listen on {listener}");
                Server::bind(listener)
                    .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            });
            for result in future::join_all(servers).await {
                if let Err(err) = result {
                    tracing::error!("server error: {err}");
                }
            }
        });
    }
}
