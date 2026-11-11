//! Shared HTTP plumbing: retries with exponential backoff and jitter,
//! `Retry-After` handling, cancellation and redacted logging hooks.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use reqwest::{RequestBuilder, StatusCode, Url};
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};

/// Retry behaviour for transient failures (429, 5xx, network errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts per request, including the first one (≥ 1).
    pub max_attempts: u32,
    /// Backoff before the second attempt; doubles with every further attempt.
    pub base_delay: Duration,
    /// Upper bound for one backoff delay.
    pub max_delay: Duration,
    /// A `Retry-After` longer than this is not waited for: the request fails
    /// with [`Error::RateLimited`] so the caller can reschedule the job.
    pub max_retry_after: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            max_retry_after: Duration::from_secs(120),
        }
    }
}

impl RetryPolicy {
    /// Backoff before attempt `attempt + 1` (attempt is 1-based), with
    /// "equal jitter": a random value in `[d/2, d]`.
    pub(crate) fn backoff(&self, attempt: u32) -> Duration {
        let exp = attempt.saturating_sub(1).min(20);
        let full = self
            .base_delay
            .saturating_mul(1u32 << exp)
            .min(self.max_delay);
        let half = full / 2;
        let jitter_range = full.saturating_sub(half).as_nanos() as u64;
        let jitter = if jitter_range == 0 {
            0
        } else {
            getrandom::u64().unwrap_or(0) % (jitter_range + 1)
        };
        half + Duration::from_nanos(jitter)
    }
}

/// How long and how often to poll for asynchronous store processing
/// (App Store Connect `assetDeliveryState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollPolicy {
    /// First wait after the commit.
    pub initial_interval: Duration,
    /// Upper bound for the (doubling) interval.
    pub max_interval: Duration,
    /// Give up after this long with [`Error::Timeout`].
    pub timeout: Duration,
}

impl Default for PollPolicy {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_secs(2),
            max_interval: Duration::from_secs(15),
            timeout: Duration::from_mins(10),
        }
    }
}

/// What happened on the wire. URLs are redacted (no query string: Apple's
/// upload URLs carry signatures); headers and bodies are never included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpEvent {
    /// HTTP method.
    pub method: String,
    /// Redacted URL: scheme, host, port and path only.
    pub url: String,
    /// 1-based attempt number.
    pub attempt: u32,
    /// Event details.
    pub kind: HttpEventKind,
}

/// Kind of [`HttpEvent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpEventKind {
    /// Request is about to be sent.
    Request,
    /// Response received.
    Response {
        /// HTTP status code.
        status: u16,
        /// Time until the full body was read.
        elapsed: Duration,
    },
    /// Network-level failure (no HTTP status).
    NetworkError {
        /// Sanitized message (no URL).
        message: String,
    },
    /// A retry is scheduled after `delay`.
    RetryScheduled {
        /// Wait before the next attempt.
        delay: Duration,
        /// Why (`"429"`, `"503"`, `"network"`).
        reason: String,
    },
}

/// Logging hook for request/response events (e.g. into the upload log).
pub trait HttpObserver: Send + Sync {
    /// Called for every [`HttpEvent`]; must not block.
    fn on_event(&self, event: &HttpEvent);
}

impl<F: Fn(&HttpEvent) + Send + Sync> HttpObserver for F {
    fn on_event(&self, event: &HttpEvent) {
        self(event);
    }
}

/// Options shared by both clients.
#[derive(Clone)]
pub struct ClientOptions {
    /// Retry behaviour.
    pub retry: RetryPolicy,
    /// Polling behaviour for asynchronous processing.
    pub poll: PollPolicy,
    /// Upload parts sent concurrently (App Store Connect allows any order
    /// and concurrency). 1 = sequential.
    pub part_concurrency: usize,
    /// Timeout for a single HTTP request.
    pub request_timeout: Duration,
    /// Optional request/response logging hook.
    pub observer: Option<Arc<dyn HttpObserver>>,
    /// Cancels all running and future calls of the client.
    pub cancel: CancellationToken,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            retry: RetryPolicy::default(),
            poll: PollPolicy::default(),
            part_concurrency: 4,
            request_timeout: Duration::from_secs(120),
            observer: None,
            cancel: CancellationToken::new(),
        }
    }
}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientOptions")
            .field("retry", &self.retry)
            .field("poll", &self.poll)
            .field("part_concurrency", &self.part_concurrency)
            .field("request_timeout", &self.request_timeout)
            .field("observer", &self.observer.as_ref().map(|_| "…"))
            .field("cancelled", &self.cancel.is_cancelled())
            .finish()
    }
}

/// Whether a request may be repeated after an ambiguous failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retry {
    /// GET, PUT, PATCH, DELETE: retry 429, 5xx and network errors.
    Idempotent,
    /// POST that creates something: retry only when the request certainly
    /// was not processed (429, 503, connection failures).
    NotIdempotent,
}

/// A successful response with its full body.
#[derive(Debug)]
pub(crate) struct Reply {
    pub status: StatusCode,
    pub body: Bytes,
}

/// Maps a non-success status (other than retried ones) to an [`Error`].
pub(crate) type ErrorMapper = fn(StatusCode, &[u8]) -> Error;

/// Request builder callback (called once per attempt).
pub(crate) type Build<'a> = &'a (dyn Fn() -> RequestBuilder + Send + Sync);

#[derive(Clone)]
pub(crate) struct Http {
    pub client: reqwest::Client,
    pub opts: ClientOptions,
}

impl fmt::Debug for Http {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Http")
            .field("opts", &self.opts)
            .finish_non_exhaustive()
    }
}

impl Http {
    pub fn new(opts: ClientOptions) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(opts.request_timeout)
            .user_agent(concat!("VitrineStudio/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::Transport {
                status: None,
                message: format!("HTTP client setup failed: {}", e.without_url()),
            })?;
        Ok(Self { client, opts })
    }

    fn emit(&self, method: &str, url: &str, attempt: u32, kind: HttpEventKind) {
        if let Some(obs) = &self.opts.observer {
            obs.on_event(&HttpEvent {
                method: method.to_owned(),
                url: url.to_owned(),
                attempt,
                kind,
            });
        }
    }

    /// Sleeps unless cancelled.
    pub async fn sleep(&self, delay: Duration) -> Result<()> {
        tokio::select! {
            () = self.opts.cancel.cancelled() => Err(Error::Cancelled),
            () = tokio::time::sleep(delay) => Ok(()),
        }
    }

    pub fn check_cancelled(&self) -> Result<()> {
        if self.opts.cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    async fn wait_retry(
        &self,
        method: &str,
        url: &str,
        attempt: u32,
        delay: Duration,
        reason: String,
    ) -> Result<()> {
        self.emit(
            method,
            url,
            attempt,
            HttpEventKind::RetryScheduled { delay, reason },
        );
        self.sleep(delay).await
    }

    /// Sends a request with retries. Non-success statuses that are not
    /// retried are mapped through `map_err`.
    pub async fn send(
        &self,
        retry: Retry,
        build: Build<'_>,
        map_err: ErrorMapper,
    ) -> Result<Reply> {
        let policy = self.opts.retry;
        let max = policy.max_attempts.max(1);
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.check_cancelled()?;
            let request = build().build().map_err(|e| Error::Config {
                message: format!("invalid request: {}", e.without_url()),
            })?;
            let method = request.method().to_string();
            let url = redact_url(request.url());
            self.emit(&method, &url, attempt, HttpEventKind::Request);
            let started = Instant::now();

            let outcome = tokio::select! {
                () = self.opts.cancel.cancelled() => return Err(Error::Cancelled),
                r = async {
                    let resp = self.client.execute(request).await?;
                    let status = resp.status();
                    let retry_after = parse_retry_after(resp.headers());
                    let body = resp.bytes().await?;
                    Ok::<_, reqwest::Error>((status, retry_after, body))
                } => r,
            };

            let (status, retry_after, body) = match outcome {
                Ok(v) => v,
                Err(e) => {
                    let connect = e.is_connect();
                    let message = e.without_url().to_string();
                    self.emit(
                        &method,
                        &url,
                        attempt,
                        HttpEventKind::NetworkError {
                            message: message.clone(),
                        },
                    );
                    let safe = retry == Retry::Idempotent || connect;
                    if safe && attempt < max {
                        let delay = policy.backoff(attempt);
                        self.wait_retry(&method, &url, attempt, delay, "network".into())
                            .await?;
                        continue;
                    }
                    return Err(Error::Transport {
                        status: None,
                        message,
                    });
                }
            };
            self.emit(
                &method,
                &url,
                attempt,
                HttpEventKind::Response {
                    status: status.as_u16(),
                    elapsed: started.elapsed(),
                },
            );

            if status.is_success() {
                return Ok(Reply { status, body });
            }

            if status == StatusCode::TOO_MANY_REQUESTS {
                let too_long = retry_after.is_some_and(|d| d > policy.max_retry_after);
                if attempt < max && !too_long {
                    let delay = retry_after.unwrap_or_else(|| policy.backoff(attempt));
                    self.wait_retry(&method, &url, attempt, delay, "429".into())
                        .await?;
                    continue;
                }
                return Err(Error::RateLimited { retry_after });
            }

            let retryable_5xx = match retry {
                Retry::Idempotent => matches!(status.as_u16(), 500 | 502 | 503 | 504),
                Retry::NotIdempotent => status == StatusCode::SERVICE_UNAVAILABLE,
            };
            if retryable_5xx && attempt < max {
                let delay = retry_after
                    .filter(|d| *d <= policy.max_retry_after)
                    .unwrap_or_else(|| policy.backoff(attempt));
                self.wait_retry(&method, &url, attempt, delay, status.as_u16().to_string())
                    .await?;
                continue;
            }

            return Err(map_err(status, &body));
        }
    }
}

/// `scheme://host[:port]/path` – drops query and fragment (signatures).
pub(crate) fn redact_url(url: &Url) -> String {
    let mut out = format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""));
    if let Some(port) = url.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    out.push_str(url.path());
    out
}

/// `Retry-After` in delta-seconds (the HTTP-date form is not used by the
/// stores; it is ignored and the backoff applies).
fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// Generic mapping for statuses without a store-specific error body.
pub(crate) fn status_error(status: StatusCode, message: String) -> Error {
    match status.as_u16() {
        401 | 403 => Error::Auth {
            status: Some(status.as_u16()),
            message,
        },
        404 => Error::NotFound { message },
        409 => Error::Conflict {
            message,
            details: Vec::new(),
        },
        429 => Error::RateLimited { retry_after: None },
        400..=499 => Error::Validation {
            message,
            details: Vec::new(),
        },
        _ => Error::Transport {
            status: Some(status.as_u16()),
            message,
        },
    }
}

/// Appends path segments (percent-encoded) to a base URL.
pub(crate) fn join_segments(base: &Url, segments: &[&str]) -> Result<Url> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| Error::config("base URL cannot be a base"))?;
        path.pop_if_empty();
        path.extend(segments);
    }
    Ok(url)
}

pub(crate) fn parse_base_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).map_err(|e| Error::config(format!("invalid base URL: {e}")))?;
    if url.cannot_be_a_base() {
        return Err(Error::config("invalid base URL"));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        let p = RetryPolicy {
            max_attempts: 10,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            max_retry_after: Duration::from_secs(5),
        };
        for attempt in 1..10 {
            let d = p.backoff(attempt);
            let full =
                (Duration::from_millis(100) * (1 << (attempt - 1))).min(Duration::from_secs(1));
            assert!(d >= full / 2 && d <= full, "attempt {attempt}: {d:?}");
        }
    }

    #[test]
    fn redaction_drops_query() {
        let url =
            Url::parse("https://store-030.blobstore.apple.com:8443/a/b?sig=SECRET&x=1#f").unwrap();
        assert_eq!(
            redact_url(&url),
            "https://store-030.blobstore.apple.com:8443/a/b"
        );
    }

    #[test]
    fn segments_are_encoded() {
        let base = Url::parse("http://h/api/").unwrap();
        let url = join_segments(&base, &["v1", "a b", "x/y"]).unwrap();
        assert_eq!(url.as_str(), "http://h/api/v1/a%20b/x%2Fy");
    }
}
