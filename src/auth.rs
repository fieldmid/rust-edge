use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::session::{self, Session};

#[derive(Debug, Deserialize)]
struct SupabaseAuthResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    user: SupabaseUser,
}

#[derive(Debug, Deserialize)]
struct SupabaseUser {
    id: String,
    email: Option<String>,
    user_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SupabaseErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrgInfo {
    pub id: String,
    pub name: String,
    pub industry: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteInfo {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
    pub site_type: Option<String>,
    pub org_id: String,
}

pub async fn login(
    supabase_url: &str,
    anon_key: &str,
    email: &str,
    password: &str,
) -> Result<Session> {
    let client = reqwest::Client::new();

    let url = format!("{}/auth/v1/token?grant_type=password", supabase_url);

    let response = client
        .post(&url)
        .header("apikey", anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .context("failed to connect to Supabase Auth")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<SupabaseErrorResponse>(&body) {
            let msg = err
                .error_description
                .or(err.msg)
                .or(err.message)
                .or(err.error)
                .unwrap_or_else(|| "unknown error".to_string());
            bail!("authentication failed ({}): {}", status, msg);
        }
        bail!("authentication failed ({}): {}", status, body);
    }

    let auth: SupabaseAuthResponse = response
        .json()
        .await
        .context("failed to parse auth response")?;

    let role = auth
        .user
        .user_metadata
        .as_ref()
        .and_then(|m| m.get("role"))
        .and_then(|v| v.as_str())
        .unwrap_or("field_worker")
        .to_string();

    let full_name = auth
        .user
        .user_metadata
        .as_ref()
        .and_then(|m| m.get("full_name").or_else(|| m.get("name")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let expires_at = chrono::Utc::now().timestamp() + auth.expires_in;

    Ok(Session {
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        expires_at,
        user_id: auth.user.id,
        email: auth.user.email.unwrap_or_default(),
        role,
        full_name,
        org_id: None,
        org_name: None,
        site_id: None,
        site_name: None,
        supabase_url: supabase_url.to_string(),
        supabase_anon_key: anon_key.to_string(),
    })
}

pub async fn refresh_session(session: &Session) -> Result<Session> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/auth/v1/token?grant_type=refresh_token",
        session.supabase_url
    );

    let response = client
        .post(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "refresh_token": session.refresh_token,
        }))
        .send()
        .await
        .context("failed to refresh token")?;

    if !response.status().is_success() {
        bail!(
            "token refresh failed ({}): session expired, please login again",
            response.status()
        );
    }

    let auth: SupabaseAuthResponse = response
        .json()
        .await
        .context("failed to parse refresh response")?;

    let expires_at = chrono::Utc::now().timestamp() + auth.expires_in;

    Ok(Session {
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        expires_at,
        ..session.clone()
    })
}

pub async fn fetch_user_profile(session: &Session) -> Result<Option<UserProfile>> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/rest/v1/user_profiles?id=eq.{}&select=*",
        session.supabase_url, session.user_id
    );

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("failed to fetch user profile")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let profiles: Vec<UserProfile> = response
        .json()
        .await
        .unwrap_or_default();

    Ok(profiles.into_iter().next())
}

#[derive(Debug, Deserialize)]
pub struct UserProfile {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub role: Option<String>,
    pub org_id: Option<String>,
    #[allow(dead_code)]
    pub site_id: Option<String>,
    #[allow(dead_code)]
    pub membership_status: Option<String>,
}

pub async fn fetch_organizations(session: &Session) -> Result<Vec<OrgInfo>> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/rest/v1/organizations?select=id,name,industry",
        session.supabase_url
    );

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("failed to fetch organizations")?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let orgs: Vec<OrgInfo> = response.json().await.unwrap_or_default();
    Ok(orgs)
}

pub async fn fetch_sites(session: &Session) -> Result<Vec<SiteInfo>> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/rest/v1/sites?select=id,name,location,site_type,org_id&active=eq.true",
        session.supabase_url
    );

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("failed to fetch sites")?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let sites: Vec<SiteInfo> = response.json().await.unwrap_or_default();
    Ok(sites)
}

pub fn ensure_valid_session(session: &Session) -> bool {
    let now = chrono::Utc::now().timestamp();
    // Consider expired if less than 60 seconds remaining
    session.expires_at > now + 60
}

pub async fn ensure_session() -> Result<Session> {
    let mut session = session::load_session()
        .context("no active session found; run `fieldmid login` first")?;

    if !ensure_valid_session(&session) {
        println!("session expired, refreshing...");
        session = refresh_session(&session).await?;
        session::save_session(&session)?;
        println!("session refreshed successfully");
    }

    Ok(session)
}

#[allow(dead_code)]
pub fn decode_jwt_role(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;

    let claims: serde_json::Value = serde_json::from_slice(&payload).ok()?;

    claims
        .get("user_metadata")
        .and_then(|m| m.get("role"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
