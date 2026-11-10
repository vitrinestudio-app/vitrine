//! Test helpers: throwaway keys generated at test time (never committed),
//! JWT checking matchers, fast retry/poll options.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrangia_publish::{ClientOptions, HttpEvent, PollPolicy, Progress, RetryPolicy};
use aws_lc_rs::encoding::{AsDer, Pkcs8V1Der};
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde_json::Value;
use wiremock::{Match, Request};

fn pem(label: &str, der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str("-----END ");
    out.push_str(label);
    out.push_str("-----\n");
    out
}

/// Fresh EC P-256 key: (PKCS#8 PEM like an ASC `.p8`, verification key).
pub fn ec_key() -> (String, DecodingKey) {
    let kp = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).unwrap();
    let der = kp.to_pkcs8v1().unwrap();
    let public = DecodingKey::from_ec_der(kp.public_key().as_ref());
    (pem("PRIVATE KEY", der.as_ref()), public)
}

/// Fresh RSA 2048 key: (PKCS#8 PEM like in a service account JSON, verification key).
pub fn rsa_key() -> (String, DecodingKey) {
    let kp = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
    let der: Pkcs8V1Der<'static> = kp.as_der().unwrap();
    let public = DecodingKey::from_rsa_der(kp.public_key().as_ref());
    (pem("PRIVATE KEY", der.as_ref()), public)
}

/// Options with tiny delays so retries and polling run fast.
pub fn fast_options() -> ClientOptions {
    ClientOptions {
        retry: RetryPolicy {
            max_attempts: 4,
            base_delay: Duration::from_millis(5),
            max_delay: Duration::from_millis(20),
            max_retry_after: Duration::from_secs(5),
        },
        poll: PollPolicy {
            initial_interval: Duration::from_millis(5),
            max_interval: Duration::from_millis(20),
            timeout: Duration::from_secs(5),
        },
        part_concurrency: 2,
        request_timeout: Duration::from_secs(10),
        ..ClientOptions::default()
    }
}

/// Collects observer events.
pub fn recorder() -> (Arc<Mutex<Vec<HttpEvent>>>, ClientOptions) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let mut opts = fast_options();
    opts.observer = Some(Arc::new(move |e: &HttpEvent| {
        sink.lock().unwrap().push(e.clone());
    }));
    (events, opts)
}

/// Collects progress updates.
#[derive(Debug, Default)]
pub struct ProgressLog(pub Mutex<Vec<Progress>>);

impl ProgressLog {
    pub fn push(&self, p: &Progress) {
        self.0.lock().unwrap().push(p.clone());
    }
    pub fn all(&self) -> Vec<Progress> {
        self.0.lock().unwrap().clone()
    }
}

/// Matches a request whose bearer token is a JWT signed by `key` that passes
/// `validation`; `check` inspects the decoded header and claims.
pub struct BearerJwt {
    pub key: DecodingKey,
    pub validation: Validation,
    pub check: fn(&jsonwebtoken::Header, &Value) -> bool,
}

impl Match for BearerJwt {
    fn matches(&self, request: &Request) -> bool {
        let Some(auth) = request.headers.get("authorization") else {
            return false;
        };
        let Some(token) = auth.to_str().ok().and_then(|s| s.strip_prefix("Bearer ")) else {
            return false;
        };
        verify(token, &self.key, &self.validation, self.check)
    }
}

pub fn verify(
    token: &str,
    key: &DecodingKey,
    validation: &Validation,
    check: fn(&jsonwebtoken::Header, &Value) -> bool,
) -> bool {
    match jsonwebtoken::decode::<Value>(token, key, validation) {
        Ok(data) => check(&data.header, &data.claims),
        Err(_) => false,
    }
}

pub fn validation(alg: Algorithm, aud: &str) -> Validation {
    let mut v = Validation::new(alg);
    v.set_audience(&[aud]);
    v.set_required_spec_claims(&["exp", "aud", "iat"]);
    v
}

/// Form field of an `application/x-www-form-urlencoded` body.
pub fn form_field(body: &[u8], name: &str) -> Option<String> {
    let url =
        reqwest::Url::parse(&format!("http://x/?{}", std::str::from_utf8(body).ok()?)).ok()?;
    url.query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}
