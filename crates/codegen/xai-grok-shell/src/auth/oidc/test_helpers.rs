//! Shared test helpers for `oidc::protocol::tests` and `oidc::login::tests`.
//! Both test modules need a mock IdP server (`start_mock_idp`), JWT
//! signing primitives (`generate_test_rsa_key`, `mock_idp_token`), and
//! the same constants. Extracted here so neither test mod has to
//! re-implement them.

use super::protocol::{Discovery, discover};

pub(super) const TEST_KID: &str = "test-kid";
pub(super) const TEST_NONCE: &str = "test-nonce-value";
pub(super) const TEST_CLIENT_ID: &str = "test-client-id";
pub(super) fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _ = jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER.install_default();
}

/// Test-only PKCS#8 RSA-2048 + JWK `n`/`e` for the mock IdP. Fixture, not
/// the `rsa` crate (RUSTSEC-2023-0071 Marvin Attack; no patched 0.9).
pub(super) fn generate_test_rsa_key() -> (String, String, String) {
    const PEM: &str = "-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDJETqse41HRBsc
7cfcq3ak4oZWFCoZlcic525A3FfO4qW9BMtRO/iXiyCCHn8JhiL9y8j5JdVP2Q9Z
IpfElcFd3/guS9w+5RqQGgCR+H56IVUyHZWtTJbKPcwWXQdNUX0rBFcsBzCRESJL
eelOEdHIjG7LRkx5l/FUvlqsyHDVJEQsHwegZ8b8C0fz0EgT2MMEdn10t6Ur1rXz
jMB/wvCg8vG8lvciXmedyo9xJ8oMOh0wUEgxziVDMMovmC+aJctcHUAYubwoGN8T
yzcvnGqL7JSh36Pwy28iPzXZ2RLhAyJFU39vLaHdljwthUaupldlNyCfa6Ofy4qN
ctlUPlN1AgMBAAECggEAdESTQjQ70O8QIp1ZSkCYXeZjuhj081CK7jhhp/4ChK7J
GlFQZMwiBze7d6K84TwAtfQGZhQ7km25E1kOm+3hIDCoKdVSKch/oL54f/BK6sKl
qlIzQEAenho4DuKCm3I4yAw9gEc0DV70DuMTR0LEpYyXcNJY3KNBOTjN5EYQAR9s
2MeurpgK2MdJlIuZaIbzSGd+diiz2E6vkmcufJLtmYUT/k/ddWvEtz+1DnO6bRHh
xuuDMeJA/lGB/EYloSLtdyCF6sII6C6slJJtgfb0bPy7l8VtL5iDyz46IKyzdyzW
tKAn394dm7MYR1RlUBEfqFUyNK7C+pVMVoTwCC2V4QKBgQD64syfiQ2oeUlLYDm4
CcKSP3RnES02bcTyEDFSuGyyS1jldI4A8GXHJ/lG5EYgiYa1RUivge4lJrlNfjyf
dV230xgKms7+JiXqag1FI+3mqjAgg4mYiNjaao8N8O3/PD59wMPeWYImsWXNyeHS
55rUKiHERtCcvdzKl4u35ZtTqQKBgQDNKnX2bVqOJ4WSqCgHRhOm386ugPHfy+8j
m6cicmUR46ND6ggBB03bCnEG9OtGisxTo/TuYVRu3WP4KjoJs2LD5fwdwJqpgtHl
yVsk45Y1Hfo+7M6lAuR8rzCi6kHHNb0HyBmZjysHWZsn79ZM+sQnLpgaYgQGRbKV
DZWlbw7g7QKBgQCl1u+98UGXAP1jFutwbPsx40IVszP4y5ypCe0gqgon3UiY/G+1
zTLp79GGe/SjI2VpQ7AlW7TI2A0bXXvDSDi3/5Dfya9ULnFXv9yfvH1QwWToySpW
Kvd1gYSoiX84/WCtjZOr0e0HmLIb0vw0hqZA4szJSqoxQgvF22EfIWaIaQKBgQCf
34+OmMYw8fEvSCPxDxVvOwW2i7pvV14hFEDYIeZKW2W1HWBhVMzBfFB5SE8yaCQy
pRfOzj9aKOCm2FjjiErVNpkQoi6jGtLvScnhZAt/lr2TXTrl8OwVkPrIaN0bG/AS
aUYxmBPCpXu3UjhfQiWqFq/mFyzlqlgvuCc9g95HPQKBgAscKP8mLxdKwOgX8yFW
GcZ0izY/30012ajdHY+/QK5lsMoxTnn0skdS+spLxaS5ZEO4qvPVb8RAoCkWMMal
2pOhmquJQVDPDLuZHdrIiKiDM20dy9sMfHygWcZjQ4WSxf/J7T9canLZIXFhHAZT
3wc9h4G8BBCtWN2TN/LsGZdB
-----END PRIVATE KEY-----
";
    // JWK `n`/`e` for the PEM above (e = 65537).
    const JWK_N: &str = "yRE6rHuNR0QbHO3H3Kt2pOKGVhQqGZXInOduQNxXzuKlvQTLUTv4l4sggh5_CYYi_cvI-SXVT9kPWSKXxJXBXd_4LkvcPuUakBoAkfh-eiFVMh2VrUyWyj3MFl0HTVF9KwRXLAcwkREiS3npThHRyIxuy0ZMeZfxVL5arMhw1SRELB8HoGfG_AtH89BIE9jDBHZ9dLelK9a184zAf8LwoPLxvJb3Il5nncqPcSfKDDodMFBIMc4lQzDKL5gvmiXLXB1AGLm8KBjfE8s3L5xqi-yUod-j8MtvIj812dkS4QMiRVN_by2h3ZY8LYVGrqZXZTcgn2ujn8uKjXLZVD5TdQ";
    const JWK_E: &str = "AQAB";
    (PEM.to_owned(), JWK_N.to_owned(), JWK_E.to_owned())
}
pub(super) async fn mock_idp_token() -> (String, String, Discovery, tokio::task::JoinHandle<()>) {
    let (issuer, handle) = start_mock_idp().await;
    let discovery = discover(&issuer).await.unwrap();
    let resp: serde_json::Value = crate::http::shared_client()
        .post(&discovery.token_endpoint)
        .form(&[("grant_type", "authorization_code")])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id_token = resp["id_token"]
        .as_str()
        .expect("mock missing id_token")
        .to_string();
    (issuer, id_token, discovery, handle)
}
pub(super) async fn start_mock_idp() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let issuer_for_discovery = issuer.clone();
    let (rsa_pem, jwk_n, jwk_e) = generate_test_rsa_key();

    #[derive(serde::Serialize)]
    struct Claims {
        sub: &'static str,
        email: &'static str,
        iss: String,
        aud: &'static str,
        nonce: &'static str,
        exp: usize,
    }

    let id_token = {
        let mut hdr = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        hdr.kid = Some(TEST_KID.to_owned());
        jsonwebtoken::encode(
            &hdr,
            &Claims {
                sub: "user-42",
                email: "test@corp.com",
                iss: issuer.clone(),
                aud: TEST_CLIENT_ID,
                nonce: TEST_NONCE,
                exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
            },
            &jsonwebtoken::EncodingKey::from_rsa_pem(rsa_pem.as_bytes()).unwrap(),
        )
        .unwrap()
    };

    let app = axum::Router::new()
        .route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let iss = issuer_for_discovery.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "authorization_endpoint": format!("{iss}/authorize"),
                        "token_endpoint": format!("{iss}/token"),
                        "jwks_uri": format!("{iss}/jwks"),
                        "id_token_signing_alg_values_supported": ["RS256"],
                    }))
                }
            }),
        )
        .route(
            "/jwks",
            axum::routing::get(move || {
                let n = jwk_n.clone();
                let e = jwk_e.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "keys": [{
                            "kty": "RSA", "alg": "RS256", "kid": TEST_KID,
                            "n": n, "e": e,
                        }]
                    }))
                }
            }),
        )
        .route(
            "/token",
            axum::routing::post(move || {
                let tok = id_token.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "access_token": "mock-access-token",
                        "refresh_token": "mock-refresh-token",
                        "id_token": tok,
                        "expires_in": 3600,
                    }))
                }
            }),
        );

    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (issuer, handle)
}
