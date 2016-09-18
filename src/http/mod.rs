//! http server for semantic engines

use iron::prelude::*;

use ::Config;

mod definition;
mod file;
mod completion;
mod ping;

use ::engine::SemanticEngine;

use iron::typemap::Key;
use iron_hmac::Hmac256Authentication;
use iron_hmac::SecretKey;

/// Errors occurring in the http module
#[derive(Debug)]
pub enum Error {
    /// Error occurred in underlying http server lib
    HttpServer(::iron::error::HttpError),
}

impl From<::iron::error::HttpError> for Error {
    fn from(err: ::iron::error::HttpError) -> Error {
        Error::HttpServer(err)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

// -------------------------------------------------------------------------------------------------
/// Iron middleware responsible for attaching a semantic engine to each request
#[derive(Debug, Clone)]
pub struct EngineProvider;

impl Key for EngineProvider {
    type Value = Box<SemanticEngine + Send + Sync>;
}

// -------------------------------------------------------------------------------------------------

/// Start the http server using the given configuration
///
/// `serve` is non-blocking.
///
/// # Example
///
/// ```no_run
/// # use libracerd::{Config};
/// let mut cfg = Config::new();
/// cfg.port = 3000;
///
/// let engine = ::libracerd::engine::Racer::new();
///
/// let mut server = ::libracerd::http::serve(&cfg, engine).unwrap();
/// // ... later
/// server.close().unwrap();
/// ```
///
pub fn serve<E: SemanticEngine + 'static>(config: &Config, engine: E) -> Result<Server> {
    use persistent::{Read, Write};

    let mut chain = Chain::new(router!(
        post "/parse_file"       => file::parse,
        post "/find_definition"  => definition::find,
        post "/list_completions" => completion::list,
        get  "/ping"             => ping::pong));

    // Get HMAC Middleware
    let (hmac_before, hmac_after) = if config.secret_file.is_some() {
        let secret = SecretKey::new(&config.read_secret_file());
        let hmac_header = "x-racerd-hmac";
        let (before, after) = Hmac256Authentication::middleware(secret, hmac_header);
        (Some(before), Some(after))
    } else {
        (None, None)
    };

    // This middleware provides a semantic engine to the request handlers
    chain.link_before(Write::<EngineProvider>::one(Box::new(engine)));

    // Body parser middlerware
    chain.link_before(Read::<::bodyparser::MaxBodyLength>::one(1024 * 1024 * 10));

    // Maybe link hmac middleware
    if let Some(hmac) = hmac_before {
        chain.link_before(hmac);
    }

    if let Some(hmac) = hmac_after {
        chain.link_after(hmac);
    }

    let app = Iron::new(chain);

    Ok(Server {
        inner: try!(app.http((&config.addr[..], config.port)))
    })
}

/// Wrapper type with information and control of the underlying HTTP server
///
/// This type can only be created via the [`serve`](fn.serve.html) function.
#[derive(Debug)]
pub struct Server {
    inner: ::iron::Listening,
}

impl Server {
    /// Stop accepting connections
    pub fn close(&mut self) -> Result<()> {
        Ok(try!(self.inner.close()))
    }

    /// Get listening address of server (eg. "127.0.0.1:59369")
    ///
    /// # Example
    /// ```no_run
    /// let mut config = ::libracerd::Config::new();
    /// config.port = 3000;
    ///
    /// let engine = ::libracerd::engine::Racer::new();
    /// let server = ::libracerd::http::serve(&config, engine).unwrap();
    ///
    /// assert_eq!(server.addr(), "0.0.0.0:3000");
    /// ```
    pub fn addr(&self) -> String {
        format!("{}", self.inner.socket)
    }
}
