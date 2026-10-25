//! The Jagex account sign-in, as the official launcher does it: an authorisation-code + PKCE leg for the launcher
//! client, then a consent leg that returns an id_token, which buys a game session and the account's characters.
//! Every Jagex address lives here so a change on their side is a one-file fix.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

pub const AUTH_URL: &str = "https://account.jagex.com/oauth2/auth";
pub const TOKEN_URL: &str = "https://account.jagex.com/oauth2/token";
pub const LAUNCHER_CLIENT: &str = "com_jagex_auth_desktop_launcher";
pub const LAUNCHER_REDIRECT: &str = "https://secure.runescape.com/m=weblogin/launcher-redirect";
pub const LAUNCHER_SCOPE: &str = "openid offline gamesso.token.create user.profile.read";
pub const CONSENT_CLIENT: &str = "1fddee4e-b100-4f4e-b2b0-097f9088f9d2";
pub const CONSENT_REDIRECT: &str = "http://localhost";
pub const CONSENT_SCOPE: &str = "openid offline";
pub const SESSIONS_URL: &str = "https://auth.jagex.com/game-session/v1/sessions";
pub const ACCOUNTS_URL: &str = "https://auth.jagex.com/game-session/v1/accounts";

pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the OS random source is unavailable");
    B64.encode(buf)
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Self {
        // 64 random bytes -> 86 base64url characters, inside RFC 7636's 43..128.
        let verifier = random_token(64);
        let challenge = B64.encode(Sha256::digest(verifier.as_bytes()));
        Self { verifier, challenge }
    }
}

pub fn launcher_auth_url(state: &str, challenge: &str) -> String {
    let mut u = Url::parse(AUTH_URL).unwrap();
    u.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", LAUNCHER_CLIENT)
        .append_pair("redirect_uri", LAUNCHER_REDIRECT)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("scope", LAUNCHER_SCOPE)
        .append_pair("state", state);
    u.into()
}

pub fn consent_url(state: &str, nonce: &str) -> String {
    let mut u = Url::parse(AUTH_URL).unwrap();
    u.query_pairs_mut()
        .append_pair("response_type", "id_token code")
        .append_pair("client_id", CONSENT_CLIENT)
        .append_pair("redirect_uri", CONSENT_REDIRECT)
        .append_pair("scope", CONSENT_SCOPE)
        .append_pair("nonce", nonce)
        .append_pair("state", state);
    u.into()
}

/// What a navigation in the sign-in window means for the flow.
#[derive(Debug, PartialEq)]
pub enum Redirect {
    /// Not one of ours: let the page load.
    Pass,
    /// The launcher leg finished with this authorisation code.
    Code(String),
    /// The consent leg finished with this id_token.
    IdToken(String),
    /// Jagex reported an error, or the state didn't match.
    Failed(String),
}

fn params(s: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(s.as_bytes()).into_owned().collect()
}

fn get<'a>(p: &'a [(String, String)], k: &str) -> Option<&'a str> {
    p.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
}

fn finish(p: &[(String, String)], state: &str, key: &str, ok: fn(String) -> Redirect) -> Redirect {
    if let Some(e) = get(p, "error") {
        let why = get(p, "error_description").unwrap_or(e);
        return Redirect::Failed(why.to_string());
    }
    if get(p, "state") != Some(state) {
        return Redirect::Failed("the sign-in reply didn't match this request (state mismatch)".into());
    }
    match get(p, key) {
        Some(v) if !v.is_empty() => ok(v.to_string()),
        _ => Redirect::Failed(format!("the sign-in reply had no {key}")),
    }
}

/// Classify a URL the sign-in window is about to open. The two redirects that carry secrets are recognised here and
/// cancelled by the caller before any request is sent, so the code and token never leave this process.
pub fn classify(url: &str, launcher_state: &str, consent_state: &str) -> Redirect {
    if let Some(rest) = url.strip_prefix(LAUNCHER_REDIRECT) {
        let q = rest.split_once('?').map(|(_, q)| q).unwrap_or("");
        return finish(&params(q), launcher_state, "code", Redirect::Code);
    }
    // The redirect page itself may forward to jagex:code=...,state=...,intent=social_auth
    if let Some(rest) = url.strip_prefix("jagex:") {
        let p: Vec<(String, String)> = rest
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        return finish(&p, launcher_state, "code", Redirect::Code);
    }
    if let Ok(u) = Url::parse(url) {
        if u.scheme() == "http" && u.host_str() == Some("localhost") {
            let mut p = params(u.fragment().unwrap_or(""));
            p.extend(params(u.query().unwrap_or("")));
            return finish(&p, consent_state, "id_token", Redirect::IdToken);
        }
    }
    Redirect::Pass
}

/// The payload of a JWT, unverified: we only read tokens Jagex just handed us over TLS.
pub fn jwt_claims(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = B64.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[derive(Deserialize)]
pub struct Tokens {
    pub id_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub account_id: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug)]
pub enum ApiError {
    /// The session is no longer accepted: the user has to sign in again.
    Expired,
    Other(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Expired => write!(f, "the Jagex session has expired; sign in again"),
            ApiError::Other(s) => write!(f, "{s}"),
        }
    }
}

fn net(e: reqwest::Error) -> ApiError {
    ApiError::Other(format!("couldn't reach Jagex: {e}"))
}

pub async fn exchange_code(http: &reqwest::Client, code: &str, verifier: &str) -> Result<Tokens, ApiError> {
    let res = http
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", LAUNCHER_CLIENT),
            ("redirect_uri", LAUNCHER_REDIRECT),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(ApiError::Other(format!("Jagex refused the sign-in code ({})", res.status())));
    }
    res.json().await.map_err(|e| ApiError::Other(format!("unexpected token reply: {e}")))
}

pub async fn create_session(http: &reqwest::Client, id_token: &str) -> Result<String, ApiError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Session {
        session_id: String,
    }
    let res = http
        .post(SESSIONS_URL)
        .json(&serde_json::json!({ "idToken": id_token }))
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(ApiError::Other(format!("Jagex refused to open a game session ({})", res.status())));
    }
    let s: Session = res.json().await.map_err(|e| ApiError::Other(format!("unexpected session reply: {e}")))?;
    Ok(s.session_id)
}

pub async fn characters(http: &reqwest::Client, session_id: &str) -> Result<Vec<Character>, ApiError> {
    let res = http.get(ACCOUNTS_URL).bearer_auth(session_id).send().await.map_err(net)?;
    match res.status().as_u16() {
        200..=299 => res.json().await.map_err(|e| ApiError::Other(format!("unexpected character list: {e}"))),
        401 | 403 => Err(ApiError::Expired),
        s => Err(ApiError::Other(format!("Jagex returned {s} for the character list"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_s256_of_verifier() {
        let p = Pkce::new();
        assert!((43..=128).contains(&p.verifier.len()));
        assert_eq!(p.challenge, B64.encode(Sha256::digest(p.verifier.as_bytes())));
        assert_ne!(Pkce::new().verifier, p.verifier);
    }

    #[test]
    fn launcher_url_carries_pkce_and_state() {
        let u = Url::parse(&launcher_auth_url("st", "ch")).unwrap();
        let q: Vec<_> = u.query_pairs().into_owned().collect();
        let has = |k: &str, v: &str| q.iter().any(|(a, b)| a == k && b == v);
        assert!(has("client_id", LAUNCHER_CLIENT));
        assert!(has("code_challenge", "ch"));
        assert!(has("code_challenge_method", "S256"));
        assert!(has("redirect_uri", LAUNCHER_REDIRECT));
        assert!(has("state", "st"));
    }

    #[test]
    fn classifies_the_launcher_redirect() {
        let url = format!("{LAUNCHER_REDIRECT}?code=abc&state=s1");
        assert_eq!(classify(&url, "s1", "s2"), Redirect::Code("abc".into()));
        assert!(matches!(classify(&url, "other", "s2"), Redirect::Failed(_)));
        assert_eq!(classify("jagex:code=xyz,state=s1,intent=social_auth", "s1", "s2"), Redirect::Code("xyz".into()));
    }

    #[test]
    fn classifies_the_consent_redirect_from_the_fragment() {
        let url = "http://localhost/#code=c&id_token=a.b.c&state=s2";
        assert_eq!(classify(url, "s1", "s2"), Redirect::IdToken("a.b.c".into()));
        let err = "http://localhost/?error=access_denied&error_description=nope&state=s2";
        assert_eq!(classify(err, "s1", "s2"), Redirect::Failed("nope".into()));
    }

    #[test]
    fn ordinary_pages_pass() {
        assert_eq!(classify("https://account.jagex.com/en-GB/login", "a", "b"), Redirect::Pass);
        assert_eq!(classify("http://localhost.evil.example/#id_token=x&state=b", "a", "b"), Redirect::Pass);
    }

    #[test]
    fn reads_jwt_claims() {
        let body = B64.encode(br#"{"sub":"u1","login_provider":"jagex"}"#);
        let claims = jwt_claims(&format!("h.{body}.s")).unwrap();
        assert_eq!(claims["sub"], "u1");
        assert!(jwt_claims("garbage").is_none());
    }
}
