//! Authentication module for bearer token validation.
//!
//! Provides constant-time token comparison, bearer header extraction,
//! and tower middleware for protecting API routes.

use axum::{
    body::Body,
    http::{header::AUTHORIZATION, Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Validates a provided token against the expected token using constant-time comparison.
///
/// This prevents timing attacks by ensuring the comparison takes the same amount
/// of time regardless of where (or if) tokens differ.
///
/// Returns `false` if either token is empty.
pub fn validate_token(provided: &str, expected: &str) -> bool {
    let provided = provided.as_bytes();
    let expected = expected.as_bytes();

    // Empty tokens are never valid
    if provided.is_empty() || expected.is_empty() {
        return false;
    }

    // Length mismatch - still compare to maintain constant time
    // We compare all bytes of the shorter string, then account for length diff
    let len_match = provided.len() == expected.len();

    // XOR accumulator: if any byte differs, result will be non-zero
    let mut diff: u8 = 0;
    for (a, b) in provided.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }

    // Both conditions must be true: same length AND all bytes match
    len_match && diff == 0
}

/// Extracts the bearer token from an Authorization header value.
///
/// Expected format: "Bearer <token>"
/// Returns `None` if the header doesn't match the expected format.
pub fn extract_bearer_token(header: &str) -> Option<&str> {
    let header = header.trim();

    // Case-insensitive "Bearer " prefix check (RFC 6750 allows case-insensitive)
    if header.len() < 7 {
        return None;
    }

    let (prefix, token) = header.split_at(7);
    if prefix.eq_ignore_ascii_case("Bearer ") {
        let token = token.trim();
        if token.is_empty() {
            None
        } else {
            Some(token)
        }
    } else {
        None
    }
}

/// Configuration for the auth middleware.
///
/// When `expected_token` is `None`, authentication is disabled and all requests pass through.
#[derive(Clone)]
pub struct AuthConfig {
    /// The expected token. If `None`, auth is disabled.
    pub expected_token: Option<Arc<String>>,
}

impl AuthConfig {
    /// Creates auth config from environment variable `BB_AUTH_TOKEN`.
    ///
    /// - If unset or empty: auth disabled (returns `None` expected_token)
    /// - If set: auth enabled, logs warning if token < 16 chars
    pub fn from_env() -> Self {
        let token = std::env::var("BB_AUTH_TOKEN").ok();
        let expected_token = token
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .map(|t| {
                if t.len() < 16 {
                    log::warn!("BB_AUTH_TOKEN is less than 16 characters - consider using a longer token");
                }
                Arc::new(t)
            });

        if expected_token.is_some() {
            log::info!("Authentication enabled for API routes");
        } else {
            log::info!("Authentication disabled (BB_AUTH_TOKEN not set)");
        }

        Self { expected_token }
    }
}

/// Tower Layer that applies authentication middleware.
#[derive(Clone)]
pub struct AuthLayer {
    config: AuthConfig,
}

impl AuthLayer {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Tower Service that validates bearer tokens on incoming requests.
#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    config: AuthConfig,
}

impl<S> Service<Request<Body>> for AuthMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // If no token configured, pass through (auth disabled)
        let Some(expected_token) = &self.config.expected_token else {
            let future = self.inner.call(req);
            return Box::pin(async move { future.await });
        };

        // Extract and validate token
        let auth_header = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        let provided_token = auth_header.and_then(extract_bearer_token);

        let is_valid = provided_token
            .map(|t| validate_token(t, expected_token))
            .unwrap_or(false);

        if is_valid {
            let future = self.inner.call(req);
            Box::pin(async move { future.await })
        } else {
            // Return 401 Unauthorized
            Box::pin(async move {
                Ok(unauthorized_response())
            })
        }
    }
}

/// Creates a 401 Unauthorized response with JSON body.
fn unauthorized_response() -> Response<Body> {
    let body = Json(serde_json::json!({"error": "Unauthorized"}));
    (StatusCode::UNAUTHORIZED, body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Unit tests ==========

    #[test]
    fn test_validate_token_matching() {
        assert!(validate_token("secret123", "secret123"));
        assert!(validate_token("a", "a"));
        assert!(validate_token(
            "very-long-token-with-special-chars!@#$%",
            "very-long-token-with-special-chars!@#$%"
        ));
    }

    #[test]
    fn test_validate_token_mismatch() {
        assert!(!validate_token("secret123", "secret124"));
        assert!(!validate_token("secret123", "SECRET123"));
        assert!(!validate_token("short", "longer"));
        assert!(!validate_token("longer", "short"));
    }

    #[test]
    fn test_validate_token_empty() {
        assert!(!validate_token("", ""));
        assert!(!validate_token("", "secret"));
        assert!(!validate_token("secret", ""));
    }

    #[test]
    fn test_extract_bearer_token_valid() {
        assert_eq!(extract_bearer_token("Bearer secret123"), Some("secret123"));
        assert_eq!(extract_bearer_token("bearer secret123"), Some("secret123"));
        assert_eq!(extract_bearer_token("BEARER secret123"), Some("secret123"));
        assert_eq!(extract_bearer_token("  Bearer secret123  "), Some("secret123"));
        assert_eq!(extract_bearer_token("Bearer   token-with-spaces  "), Some("token-with-spaces"));
    }

    #[test]
    fn test_extract_bearer_token_invalid() {
        assert_eq!(extract_bearer_token(""), None);
        assert_eq!(extract_bearer_token("Basic secret123"), None);
        assert_eq!(extract_bearer_token("Bearer"), None);
        assert_eq!(extract_bearer_token("Bearer "), None);
        assert_eq!(extract_bearer_token("Bearersecret123"), None);
        assert_eq!(extract_bearer_token("secret123"), None);
    }

    // ========== Middleware integration tests ==========

    mod middleware {
        use super::*;
        use axum::{body::Body, routing::get, Router};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        async fn ok_handler() -> &'static str {
            "ok"
        }

        fn test_app(expected_token: Option<&str>) -> Router {
            let config = AuthConfig {
                expected_token: expected_token.map(|t| Arc::new(t.to_string())),
            };
            Router::new()
                .route("/test", get(ok_handler))
                .layer(AuthLayer::new(config))
        }

        #[tokio::test]
        async fn auth_disabled_allows_all_requests() {
            let app = test_app(None);

            // No header at all
            let req = Request::builder()
                .uri("/test")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);

            // Random header (should still pass when disabled)
            let req = Request::builder()
                .uri("/test")
                .header(AUTHORIZATION, "Bearer wrong")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn valid_token_returns_200() {
            let app = test_app(Some("secret-token-1234"));

            let req = Request::builder()
                .uri("/test")
                .header(AUTHORIZATION, "Bearer secret-token-1234")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn valid_token_case_insensitive_bearer() {
            let app = test_app(Some("secret-token-1234"));

            // lowercase "bearer"
            let req = Request::builder()
                .uri("/test")
                .header(AUTHORIZATION, "bearer secret-token-1234")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn invalid_token_returns_401() {
            let app = test_app(Some("secret-token-1234"));

            let req = Request::builder()
                .uri("/test")
                .header(AUTHORIZATION, "Bearer wrong-token")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn missing_header_returns_401() {
            let app = test_app(Some("secret-token-1234"));

            let req = Request::builder()
                .uri("/test")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn malformed_bearer_returns_401() {
            let app = test_app(Some("secret-token-1234"));

            // Missing "Bearer " prefix
            let req = Request::builder()
                .uri("/test")
                .header(AUTHORIZATION, "secret-token-1234")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

            // Basic auth instead of bearer
            let req = Request::builder()
                .uri("/test")
                .header(AUTHORIZATION, "Basic dXNlcjpwYXNz")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        #[tokio::test]
        async fn unauthorized_response_is_json() {
            let app = test_app(Some("secret-token-1234"));

            let req = Request::builder()
                .uri("/test")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();

            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["error"], "Unauthorized");
        }
    }
}
