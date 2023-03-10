use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{FromRequest, MatchedPath},
    http::{HeaderMap, Method, Request},
};
use hyper::body::{self, Buf, HttpBody};
use std::{
    convert::Infallible,
    io::Read,
    net::{IpAddr, SocketAddr},
    ops::{Deref, DerefMut},
    sync::LazyLock,
};
use toml::value::Table;
use tower_cookies::{Cookie, Cookies, Key};
use zino_core::{
    application::Application,
    channel::CloudEvent,
    error::Error,
    extend::HeaderMapExt,
    request::{Context, RequestContext},
    response::Rejection,
    state::State,
    Map,
};

/// An HTTP request extractor for `axum`.
pub struct AxumExtractor<T>(pub(crate) T);

impl<T> Deref for AxumExtractor<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for AxumExtractor<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RequestContext for AxumExtractor<Request<Body>> {
    type Method = Method;
    type Headers = HeaderMap;

    #[inline]
    fn request_method(&self) -> &Self::Method {
        self.method()
    }

    #[inline]
    fn request_path(&self) -> &str {
        self.uri().path()
    }

    #[inline]
    fn matched_route(&self) -> &str {
        if let Some(path) = self.extensions().get::<MatchedPath>() {
            path.as_str()
        } else {
            self.uri().path()
        }
    }

    #[inline]
    fn query_string(&self) -> Option<&str> {
        self.uri().query()
    }

    #[inline]
    fn header_map(&self) -> &Self::Headers {
        self.headers()
    }

    #[inline]
    fn header_map_mut(&mut self) -> &mut Self::Headers {
        self.headers_mut()
    }

    #[inline]
    fn get_header(&self, name: &str) -> Option<&str> {
        self.headers()
            .get(name)?
            .to_str()
            .inspect_err(|err| tracing::error!("{err}"))
            .ok()
    }

    #[inline]
    fn get_context(&self) -> Option<&Context> {
        self.extensions().get::<Context>()
    }

    #[inline]
    fn get_cookie(&self, name: &str) -> Option<Cookie<'static>> {
        let cookies = self.extensions().get::<Cookies>()?;
        let key = LazyLock::force(&COOKIE_PRIVATE_KEY);
        let signed_cookies = cookies.signed(key);
        signed_cookies.get(name)
    }

    #[inline]
    fn add_cookie(&self, cookie: Cookie<'static>) {
        self.extensions().get::<Cookies>().map(|cookies| {
            let key = LazyLock::force(&COOKIE_PRIVATE_KEY);
            let signed_cookies = cookies.signed(key);
            signed_cookies.add(cookie);
        });
    }

    #[inline]
    fn client_ip(&self) -> Option<IpAddr> {
        self.header_map().get_client_ip().or_else(|| {
            self.extensions()
                .get::<SocketAddr>()
                .map(|socket| socket.ip())
        })
    }

    #[inline]
    fn config(&self) -> &Table {
        let state = self
            .extensions()
            .get::<State>()
            .expect("the request extension `State` does not exist");
        state.config()
    }

    #[inline]
    fn state_data(&self) -> &Map {
        let state = self
            .extensions()
            .get::<State>()
            .expect("the request extension `State` does not exist");
        state.data()
    }

    #[inline]
    fn state_data_mut(&mut self) -> &mut Map {
        let state = self
            .extensions_mut()
            .get_mut::<State>()
            .expect("the request extension `State` does not exist");
        state.data_mut()
    }

    #[inline]
    fn try_send(&self, message: CloudEvent) -> Result<(), Rejection> {
        crate::channel::axum_channel::MessageChannel::shared()
            .try_send(message)
            .map_err(Rejection::internal_server_error)
    }

    async fn read_body_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let buffer_size = self.size_hint().lower().try_into().unwrap_or(128);
        let body = body::aggregate(self.body_mut()).await?;
        let mut bytes = Vec::with_capacity(buffer_size);
        body.reader().read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

#[async_trait]
impl FromRequest<(), Body> for AxumExtractor<Request<Body>> {
    type Rejection = Infallible;

    #[inline]
    async fn from_request(req: Request<Body>, _state: &()) -> Result<Self, Self::Rejection> {
        Ok(AxumExtractor(req))
    }
}

/// Private key for cookie signing.
static COOKIE_PRIVATE_KEY: LazyLock<Key> = LazyLock::new(|| {
    let secret_key = crate::AxumCluster::secret_key();
    Key::try_from(secret_key).unwrap_or_else(|_| Key::generate())
});
