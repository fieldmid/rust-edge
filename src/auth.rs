use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::session::{self, Session};

// ─── Error Messages (prod-quality, user-friendly) ───────────────────────────

const ERR_NO_SESSION: &str = "\x1b[31m✗ Not logged in.\x1b[0m

  Run \x1b[1mfieldmid login\x1b[0m to authenticate.
  This opens your browser for secure sign-in — no password enters the terminal.";

const ERR_SESSION_EXPIRED: &str = "\x1b[33m⚠ Session expired.\x1b[0m

  Your authentication token has expired and could not be refreshed.
  Run \x1b[1mfieldmid login\x1b[0m to sign in again.";

const ERR_NETWORK: &str = "\x1b[31m✗ Network error.\x1b[0m

  Could not reach FieldMid services. Please check:
    • Your internet connection is active
    • Firewall is not blocking outbound HTTPS
    • VPN/proxy settings are correct

  If the problem persists, check https://status.fieldmid.com";

const ERR_USER_NOT_FOUND: &str = "\x1b[31m✗ Account not found.\x1b[0m

  No FieldMid account exists for this email.
  \x1b[1mTo create an account:\x1b[0m
    1. Visit your organization's FieldMid dashboard
    2. Click \x1b[1mCreate Account\x1b[0m
    3. Verify your email, then try \x1b[1mfieldmid login\x1b[0m again";

const ERR_INVALID_CREDENTIALS: &str = "\x1b[31m✗ Invalid credentials.\x1b[0m

  The email or password is incorrect.
  \x1b[2mForgot your password? Visit the FieldMid dashboard → Forgot Password\x1b[0m";

const ERR_EMAIL_NOT_VERIFIED: &str = "\x1b[33m⚠ Email not verified.\x1b[0m

  Your account exists but the email is not yet verified.
  Check your inbox for a verification link, then try again.
  \x1b[2mDidn't receive it? Sign in to the dashboard to resend.\x1b[0m";

const ERR_TOO_MANY_REQUESTS: &str = "\x1b[33m⚠ Too many attempts.\x1b[0m

  You've made too many login attempts. Please wait a minute and try again.";

const ERR_BROWSER_TIMEOUT: &str = "\x1b[33m⚠ Login timed out.\x1b[0m

  The browser login was not completed within 10 minutes.
  Run \x1b[1mfieldmid login\x1b[0m to try again.";

const ERR_BROWSER_OPEN: &str = "\x1b[33m⚠ Could not open browser.\x1b[0m

  Please open this URL manually in your browser:";

// ─── Types ──────────────────────────────────────────────────────────────────

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

#[derive(Debug, Deserialize)]
struct CliAuthStatusResponse {
    status: String,
    #[allow(dead_code)]
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CliAuthConsumeResponse {
    #[allow(dead_code)]
    status: String,
    access_token: Option<String>,
    user_id: Option<String>,
    user_email: Option<String>,
    user_metadata: Option<serde_json::Value>,
}

// ─── Browser-based Login Flow ───────────────────────────────────────────────

pub async fn browser_login(
    supabase_url: &str,
    anon_key: &str,
    core_dashboard_url: &str,
) -> Result<Session> {
    let client = reqwest::Client::new();
    let session_id = uuid::Uuid::new_v4().to_string();
    let verification_code = generate_verification_code();
    let hostname = gethostname();
    let token_name = format!("cli_{}_{}", hostname, chrono::Utc::now().timestamp());

    // Step 1: Create pending session on server
    let create_url = format!(
        "{}/functions/v1/cli-auth?action=create",
        supabase_url
    );

    let create_resp = client
        .post(&create_url)
        .header("apikey", anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "session_id": session_id,
            "verification_code": verification_code,
            "token_name": token_name,
        }))
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("{}", ERR_NETWORK))?;

    if !create_resp.status().is_success() {
        let body = create_resp.text().await.unwrap_or_default();
        bail!("Failed to initiate login session: {}", body);
    }

    // Step 2: Open browser
    let login_url = format!(
        "{}/auth/cli-login?session_id={}",
        core_dashboard_url, session_id
    );

    println!();
    println!(
        "  \x1b[1mHello from FieldMid!\x1b[0m Press \x1b[1mEnter\x1b[0m to open browser and login automatically."
    );
    println!();

    // Show the link in case browser doesn't open
    println!(
        "  \x1b[2mHere is your login link in case browser did not open:\x1b[0m"
    );
    println!("  \x1b[36m{}\x1b[0m", login_url);
    println!();

    // Try to open browser
    if let Err(_) = open_browser(&login_url) {
        println!("{}", ERR_BROWSER_OPEN);
        println!("  \x1b[36m{}\x1b[0m", login_url);
        println!();
    }

    // Step 3: Show verification code
    println!(
        "  \x1b[1mYour verification code:\x1b[0m \x1b[32;1m{}\x1b[0m",
        verification_code
    );
    println!();
    println!(
        "  \x1b[2mPaste this code into the browser page to authorize this device.\x1b[0m"
    );
    println!();

    // Step 4: Poll for authorization
    print!("  Waiting for browser authorization");

    let poll_url = format!(
        "{}/functions/v1/cli-auth?action=status&session_id={}",
        supabase_url, session_id
    );

    let timeout = std::time::Duration::from_secs(600); // 10 minutes
    let poll_interval = std::time::Duration::from_secs(2);
    let start = std::time::Instant::now();
    let mut dots = 0;

    loop {
        if start.elapsed() > timeout {
            println!();
            bail!("{}", ERR_BROWSER_TIMEOUT);
        }

        tokio::time::sleep(poll_interval).await;
        dots += 1;
        if dots % 3 == 0 {
            print!(".");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        let resp = client
            .get(&poll_url)
            .header("apikey", anon_key)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(_) => continue, // Retry on network hiccup
        };

        if !resp.status().is_success() {
            continue;
        }

        let status: CliAuthStatusResponse = match resp.json().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        match status.status.as_str() {
            "authorized" => {
                println!();
                break;
            }
            "expired" => {
                println!();
                bail!("{}", ERR_BROWSER_TIMEOUT);
            }
            "pending" => continue,
            other => {
                println!();
                bail!("Unexpected session status: {}", other);
            }
        }
    }

    // Step 5: Consume the session to get tokens
    let consume_url = format!(
        "{}/functions/v1/cli-auth?action=consume&session_id={}",
        supabase_url, session_id
    );

    let consume_resp = client
        .get(&consume_url)
        .header("apikey", anon_key)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("{}", ERR_NETWORK))?;

    if !consume_resp.status().is_success() {
        bail!("Failed to retrieve login credentials. Please try again.");
    }

    let consumed: CliAuthConsumeResponse = consume_resp
        .json()
        .await
        .context("Failed to parse login response")?;

    let access_token = consumed
        .access_token
        .ok_or_else(|| anyhow::anyhow!("Server did not return an access token"))?;

    let role = consumed
        .user_metadata
        .as_ref()
        .and_then(|m| m.get("role"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| decode_jwt_role(&access_token))
        .unwrap_or_else(|| "field_worker".to_string());

    let full_name = consumed
        .user_metadata
        .as_ref()
        .and_then(|m| m.get("full_name").or_else(|| m.get("name")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Decode token expiry from JWT
    let expires_at = decode_jwt_exp(&access_token)
        .unwrap_or_else(|| chrono::Utc::now().timestamp() + 3600);

    println!(
        "  \x1b[32m✓\x1b[0m Token \x1b[1m{}\x1b[0m created successfully.",
        token_name
    );
    println!();

    Ok(Session {
        access_token,
        refresh_token: String::new(), // Browser flow doesn't provide refresh_token directly
        expires_at,
        user_id: consumed.user_id.unwrap_or_default(),
        email: consumed.user_email.unwrap_or_default(),
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

// ─── Password Login (fallback) ──────────────────────────────────────────────

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
        .map_err(|_| anyhow::anyhow!("{}", ERR_NETWORK))?;

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

            let friendly = classify_auth_error(&msg, status.as_u16());
            bail!("{}", friendly);
        }
        bail!(
            "\x1b[31m✗ Authentication failed\x1b[0m (HTTP {})\n\n  {}",
            status,
            body
        );
    }

    let auth: SupabaseAuthResponse = response
        .json()
        .await
        .context("Failed to parse authentication response")?;

    let role = auth
        .user
        .user_metadata
        .as_ref()
        .and_then(|m| m.get("role"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| decode_jwt_role(&auth.access_token))
        .unwrap_or_else(|| "field_worker".to_string());

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

// ─── Token Refresh ──────────────────────────────────────────────────────────

pub async fn refresh_session(session: &Session) -> Result<Session> {
    if session.refresh_token.is_empty() {
        bail!("{}", ERR_SESSION_EXPIRED);
    }

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
        .map_err(|_| anyhow::anyhow!("{}", ERR_NETWORK))?;

    if !response.status().is_success() {
        bail!("{}", ERR_SESSION_EXPIRED);
    }

    let auth: SupabaseAuthResponse = response
        .json()
        .await
        .context("Failed to parse token refresh response")?;

    let expires_at = chrono::Utc::now().timestamp() + auth.expires_in;

    Ok(Session {
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        expires_at,
        ..session.clone()
    })
}

// ─── Profile & Org Fetching ─────────────────────────────────────────────────

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
        .context("Failed to fetch user profile")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let profiles: Vec<UserProfile> = response.json().await.unwrap_or_default();

    Ok(profiles.into_iter().next())
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
        .context("Failed to fetch organizations")?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let orgs: Vec<OrgInfo> = response.json().await.unwrap_or_default();
    Ok(orgs)
}

#[derive(Debug, Deserialize)]
struct JoinRequestInfo {
    #[allow(dead_code)]
    id: String,
    requester_user_id: String,
    requested_role: Option<String>,
    status: Option<String>,
    #[allow(dead_code)]
    message: Option<String>,
    created_at: Option<String>,
}

pub async fn fetch_org_users(session: &Session) -> Result<(Vec<OrgUserProfile>, usize)> {
    let client = reqwest::Client::new();
    let org_id = session.org_id.as_deref().unwrap_or("");
    if org_id.is_empty() {
        return Ok((Vec::new(), 0));
    }

    // Fetch all user_profiles in this org
    let url = format!(
        "{}/rest/v1/user_profiles?org_id=eq.{}&select=id,full_name,email,role,membership_status",
        session.supabase_url, org_id
    );

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("Failed to fetch org users")?;

    if !response.status().is_success() {
        return Ok((Vec::new(), 0));
    }

    let profiles: Vec<OrgUserProfile> = response.json().await.unwrap_or_default();
    let count = profiles.len();
    Ok((profiles, count))
}

#[derive(Debug, Deserialize, Clone)]
pub struct OrgUserProfile {
    pub id: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub membership_status: Option<String>,
}

pub async fn fetch_join_requests(session: &Session) -> Result<Vec<JoinRequestDisplay>> {
    let client = reqwest::Client::new();
    let org_id = session.org_id.as_deref().unwrap_or("");
    if org_id.is_empty() {
        return Ok(Vec::new());
    }

    let url = format!(
        "{}/rest/v1/org_join_requests?org_id=eq.{}&status=neq.cancelled&select=id,requester_user_id,requested_role,status,message,created_at&order=created_at.desc",
        session.supabase_url, org_id
    );

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("Failed to fetch join requests")?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let requests: Vec<JoinRequestInfo> = response.json().await.unwrap_or_default();

    // Fetch profiles for requesters
    let user_ids: Vec<&str> = requests.iter()
        .map(|r| r.requester_user_id.as_str())
        .collect();

    let mut profiles_map = std::collections::HashMap::new();
    for user_id in &user_ids {
        let profile_url = format!(
            "{}/rest/v1/user_profiles?id=eq.{}&select=id,full_name,email",
            session.supabase_url, user_id
        );
        if let Ok(resp) = client
            .get(&profile_url)
            .header("apikey", &session.supabase_anon_key)
            .header("Authorization", format!("Bearer {}", session.access_token))
            .send()
            .await
        {
            if let Ok(profiles) = resp.json::<Vec<OrgUserProfile>>().await {
                if let Some(profile) = profiles.into_iter().next() {
                    profiles_map.insert(profile.id.clone(), profile);
                }
            }
        }
    }

    let display: Vec<JoinRequestDisplay> = requests.into_iter().map(|r| {
        let profile = profiles_map.get(&r.requester_user_id);
        let name = profile
            .and_then(|p| p.full_name.as_deref())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| profile.and_then(|p| p.email.as_deref()).unwrap_or("Unknown"))
            .to_string();
        let email = profile
            .and_then(|p| p.email.clone())
            .unwrap_or_else(|| "No email".to_string());

        JoinRequestDisplay {
            id: r.id,
            name,
            email,
            requested_role: r.requested_role.unwrap_or_else(|| "field_worker".to_string()),
            status: r.status.unwrap_or_else(|| "pending".to_string()),
            created_at: r.created_at,
        }
    }).collect();

    Ok(display)
}

#[derive(Debug, Clone)]
pub struct JoinRequestDisplay {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub email: String,
    pub requested_role: String,
    pub status: String,
    pub created_at: Option<String>,
}

pub async fn decide_join_request(
    session: &Session,
    request_id: &str,
    decision: &str,
) -> Result<()> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/rest/v1/rpc/decide_org_join_request",
        session.supabase_url
    );

    let response = client
        .post(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "p_request_id": request_id,
            "p_decision": decision,
        }))
        .send()
        .await
        .context("Failed to send join request decision")?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("Failed to {} request: {}", decision, body);
    }

    Ok(())
}

pub async fn fetch_sites(session: &Session) -> Result<Vec<SiteInfo>> {
    let client = reqwest::Client::new();

    let mut url = format!(
        "{}/rest/v1/sites?select=id,name,location,site_type,org_id&active=eq.true",
        session.supabase_url
    );
    if let Some(org_id) = &session.org_id {
        url.push_str("&org_id=eq.");
        url.push_str(org_id);
    }

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("Failed to fetch sites")?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let sites: Vec<SiteInfo> = response.json().await.unwrap_or_default();
    Ok(sites)
}

pub async fn fetch_supervisor_site_count(session: &Session) -> Result<usize> {
    #[derive(Deserialize)]
    struct SupervisorAssignment {
        site_id: Option<String>,
    }

    let client = reqwest::Client::new();
    let url = format!(
        "{}/rest/v1/supervisors?select=site_id&user_id=eq.{}",
        session.supabase_url, session.user_id
    );

    let response = client
        .get(&url)
        .header("apikey", &session.supabase_anon_key)
        .header("Authorization", format!("Bearer {}", session.access_token))
        .send()
        .await
        .context("Failed to fetch supervisor site assignments")?;

    if !response.status().is_success() {
        return Ok(0);
    }

    let assignments: Vec<SupervisorAssignment> = response.json().await.unwrap_or_default();
    let unique = assignments
        .into_iter()
        .filter_map(|assignment| assignment.site_id)
        .collect::<std::collections::HashSet<_>>();

    Ok(unique.len())
}

// ─── Session Validation ─────────────────────────────────────────────────────

pub fn ensure_valid_session(session: &Session) -> bool {
    let now = chrono::Utc::now().timestamp();
    session.expires_at > now + 60
}

pub async fn ensure_session() -> Result<Session> {
    let mut session = session::load_session().map_err(|_| anyhow::anyhow!("{}", ERR_NO_SESSION))?;

    if !ensure_valid_session(&session) {
        eprintln!("  \x1b[2mSession expired, refreshing...\x1b[0m");
        session = refresh_session(&session).await?;
        session::save_session(&session)?;
        eprintln!("  \x1b[32m✓\x1b[0m Session refreshed.");
    }

    Ok(session)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn classify_auth_error(message: &str, status_code: u16) -> String {
    let lower = message.to_lowercase();

    if lower.contains("invalid login credentials")
        || lower.contains("invalid credentials")
        || lower.contains("invalid password")
    {
        return ERR_INVALID_CREDENTIALS.to_string();
    }

    if lower.contains("user not found") || lower.contains("no user found") {
        return ERR_USER_NOT_FOUND.to_string();
    }

    if lower.contains("email not confirmed") || lower.contains("email not verified") {
        return ERR_EMAIL_NOT_VERIFIED.to_string();
    }

    if lower.contains("too many requests") || status_code == 429 {
        return ERR_TOO_MANY_REQUESTS.to_string();
    }

    format!(
        "\x1b[31m✗ Authentication failed\x1b[0m\n\n  {}",
        message
    )
}

fn generate_verification_code() -> String {
    let bytes: [u8; 4] = rand_bytes();
    hex::encode(bytes)
}

fn rand_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("failed to generate random bytes");
    buf
}

fn gethostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to open browser")?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to open browser")?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to open browser")?;
    }
    Ok(())
}

fn decode_jwt_exp(token: &str) -> Option<i64> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    claims.get("exp").and_then(|v| v.as_i64())
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
        .or_else(|| {
            claims
                .get("app_metadata")
                .and_then(|m| m.get("role"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string())
}
