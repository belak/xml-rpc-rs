use Request;

use std::io::Read;
use std::error::Error;

/// Request and response transport abstraction.
///
/// The `Transport` trait provides a way to send a `Request` to a server and to receive the
/// corresponding response. A `Transport` implementor is passed to `Request::call` in order to use
/// it to perform that request.
///
/// The most commonly used transport is simple HTTP: If the `reqwest` feature is enabled (it is by
/// default), the reqwest `Client` will implement this trait and send the XML-RPC `Request` via
/// HTTP.
///
/// You can implement this trait for your own types if you want to customize how requests are sent.
/// You can modify HTTP headers or wrap requests in a completely different protocol.
pub trait Transport {
    // FIXME replace with `impl Trait` when stable
    /// The response stream returned by `transmit`.
    type Stream: Read;

    /// Transmits an XML-RPC request and returns the server's response.
    ///
    /// The response is returned as a `Self::Stream` - some type implementing the `Read` trait. The
    /// library will read all of the data and parse it as a response. It must be UTF-8 encoded XML,
    /// otherwise the call will fail.
    ///
    /// If a transport error occurs, this should return it as a boxed error - the library will
    /// interpret it as a transport error and return an appropriate `RequestError`.
    fn transmit(self, request: &Request) -> Result<Self::Stream, Box<Error>>;
}

/// Contains `Transport` implementations for types of the `reqwest` crate.
#[cfg(feature = "reqwest")]
mod reqwest {
    extern crate reqwest;

    use {Request, Transport};
    use self::reqwest::{Client, RequestBuilder, Url, mime};
    use self::reqwest::header::{ContentType, ContentLength, UserAgent};

    use std::error::Error;

    /// Use a `RequestBuilder` as the transport.
    ///
    /// The request will be sent as specified in the XML-RPC specification: A default `User-Agent`
    /// will be set, along with the correct `Content-Type` and `Content-Length`.
    impl Transport for RequestBuilder {
        type Stream = reqwest::Response;

        fn transmit(mut self, request: &Request) -> Result<Self::Stream, Box<Error>> {
            // First, build the body XML
            let mut body = Vec::new();
            // This unwrap never panics as we are using `Vec<u8>` as a `Write` implementor,
            // and not doing anything else that could return an `Err` in `write_as_xml()`.
            request.write_as_xml(&mut body).unwrap();

            // Set all required request headers
            // NB: The `Host` header is also required, but reqwest adds it automatically, since
            // HTTP/1.1 requires it.
            let builder = self
                .header(UserAgent::new("Rust xmlrpc"))
                .header(ContentType("text/xml; charset=utf-8".parse().unwrap()))
                .header(ContentLength(body.len() as u64));

            let response = builder
                .body(body)
                .send()?
                .error_for_status()?;

            // Check response headers
            // "The Content-Type is text/xml. Content-Length must be present and correct."
            if let Some(content) = response.headers().get::<ContentType>() {
                // (we ignore this if the header is missing completely)
                match (content.type_(), content.subtype()) {
                    (mime::TEXT, mime::XML) => {},
                    (ty, sub) => return Err(
                        format!("expected Content-Type 'text/xml', got '{}/{}'", ty, sub).into()
                    ),
                }
            }

            response.headers().get::<ContentLength>()
                .ok_or_else(|| format!("expected Content-Length header, but none was found"))?;

            Ok(response)
        }
    }

    /// Convenience implementation for `Url`. Creates a `Client` and performs a `POST` request to
    /// the URL.
    // (this isn't actually that convenient - to parse a URL to have to deal with another layer of
    // errors)
    impl Transport for Url {
        type Stream = reqwest::Response;

        fn transmit(self, request: &Request) -> Result<Self::Stream, Box<Error>> {
            Client::new().post(self).transmit(request)
        }
    }
}

// TODO: integration test - can you usefully implement this trait from outside?
// Also test reported use cases:
// - Cookie support (missing in reqwest!),
// - Custom useragent
// - "XML-RPC wrapped in SCGI over a unix domain socket"
