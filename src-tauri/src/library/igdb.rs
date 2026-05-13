//! IGDB (Internet Game Database) HTTP client.
//!
//! - Auth: Twitch OAuth2 client_credentials grant → bearer token with TTL.
//! - Queries: Apicalypse syntax (plain text, POSTed as request body).
//! - Rate limit: IGDB enforces 4 req/sec / 8 concurrent. We pace at 4 req/sec
//!   evenly via [`tokio::time::interval`] inside [`IgdbClient::throttle`].
//!
//! Endpoints we use:
//!   POST https://id.twitch.tv/oauth2/token         (auth)
//!   POST https://api.igdb.com/v4/games             (search)

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::sync::Mutex;

use super::types::IgdbMetadata;

const AUTH_URL:  &str = "https://id.twitch.tv/oauth2/token";
const GAMES_URL: &str = "https://api.igdb.com/v4/games";
/// 4 req/sec → 250 ms between requests.
const MIN_REQUEST_GAP: Duration = Duration::from_millis(260);

#[derive(Debug, Clone)]
pub struct IgdbClient {
    http:          reqwest::Client,
    client_id:     String,
    client_secret: String,
    inner:         Arc<Mutex<ClientInner>>,
}

#[derive(Debug, Default)]
struct ClientInner {
    token:      Option<String>,
    expires_at: Option<Instant>,
    last_call:  Option<Instant>,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in:   u64,
}

/// Raw IGDB game response. Public so the enrichment step can map it into
/// [`IgdbMetadata`] without going through serde_json::Value gymnastics.
#[derive(Debug, Deserialize, Clone)]
pub struct IgdbGame {
    pub id:                  u64,
    pub name:                String,
    #[serde(default)] pub summary:             Option<String>,
    #[serde(default)] pub first_release_date:  Option<i64>,
    #[serde(default)] pub total_rating:        Option<f64>,
    #[serde(default)] pub cover:               Option<IgdbCover>,
    #[serde(default)] pub platforms:           Vec<IgdbPlatformRef>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IgdbCover {
    #[serde(default)] pub url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IgdbPlatformRef {
    #[serde(default)] pub name: Option<String>,
}

impl IgdbClient {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("AbyssSingularity/0.1 (+local)")
            .timeout(Duration::from_secs(15))
            .build()
            .context("constructing reqwest client")?;
        Ok(Self {
            http,
            client_id:     client_id.into(),
            client_secret: client_secret.into(),
            inner:         Arc::new(Mutex::new(ClientInner::default())),
        })
    }

    /// Returns a valid bearer token, refreshing if missing/expired.
    pub async fn token(&self) -> Result<String> {
        // Re-use the existing token if still fresh (with a 60s safety margin).
        {
            let guard = self.inner.lock().await;
            if let (Some(t), Some(exp)) = (&guard.token, guard.expires_at) {
                if exp > Instant::now() + Duration::from_secs(60) {
                    return Ok(t.clone());
                }
            }
        }

        let resp = self
            .http
            .post(AUTH_URL)
            .query(&[
                ("client_id",     self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("grant_type",    "client_credentials"),
            ])
            .send()
            .await
            .context("requesting IGDB token")?
            .error_for_status()
            .context("IGDB auth returned non-2xx")?;
        let auth: AuthResponse = resp.json().await.context("parsing IGDB auth response")?;

        let mut guard = self.inner.lock().await;
        guard.token      = Some(auth.access_token.clone());
        guard.expires_at = Some(Instant::now() + Duration::from_secs(auth.expires_in));
        Ok(auth.access_token)
    }

    /// Pace outbound requests to stay within IGDB's 4 req/s budget.
    pub async fn throttle(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(last) = guard.last_call {
            let elapsed = last.elapsed();
            if elapsed < MIN_REQUEST_GAP {
                tokio::time::sleep(MIN_REQUEST_GAP - elapsed).await;
            }
        }
        guard.last_call = Some(Instant::now());
    }

    /// Search by name. Returns the top-N matches, ordered by IGDB's own
    /// relevance scoring.
    pub async fn search_game(&self, name: &str, limit: u32) -> Result<Vec<IgdbGame>> {
        let token = self.token().await?;
        self.throttle().await;
        let body = build_search_query(name, limit);
        let resp = self
            .http
            .post(GAMES_URL)
            .header("Client-ID", &self.client_id)
            .header("Authorization", format!("Bearer {token}"))
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(body)
            .send()
            .await
            .context("IGDB /v4/games request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("IGDB /v4/games returned {status}: {text}"));
        }
        let games: Vec<IgdbGame> = resp.json().await.context("parsing IGDB /v4/games response")?;
        Ok(games)
    }
}

/// Build an Apicalypse query for searching games by name. Public for
/// unit testing — the formatting is finicky and easy to regress.
pub fn build_search_query(name: &str, limit: u32) -> String {
    // Quotes around the search term are required; escape any embedded ".
    let escaped = name.replace('"', "\\\"");
    format!(
        "fields name,summary,cover.url,first_release_date,total_rating,platforms.name; \
         search \"{escaped}\"; \
         limit {limit};"
    )
}

/// IGDB returns protocol-relative URLs like `//images.igdb.com/.../t_thumb/abc.jpg`.
/// We want absolute https URLs and a higher-resolution variant suitable for a
/// game card. Sizes documented at https://api-docs.igdb.com/#images.
pub fn upgrade_cover_url(raw: &str) -> String {
    let with_scheme = if raw.starts_with("//") {
        format!("https:{raw}")
    } else if !raw.starts_with("http") {
        format!("https://{raw}")
    } else {
        raw.to_string()
    };
    // Swap any IGDB size token for the higher-res cover variant. Idempotent
    // if the URL is already at the target size.
    with_scheme.replace("/t_thumb/", "/t_cover_big_2x/")
}

/// Map an [`IgdbGame`] into our cache-shaped [`IgdbMetadata`].
pub fn to_metadata(game: IgdbGame) -> IgdbMetadata {
    let release_year = game.first_release_date.and_then(unix_seconds_to_year);
    let cover_url = game
        .cover
        .as_ref()
        .and_then(|c| c.url.as_deref())
        .map(upgrade_cover_url);
    let platforms = game
        .platforms
        .into_iter()
        .filter_map(|p| p.name)
        .collect();
    IgdbMetadata {
        igdb_id:      game.id,
        name:         game.name,
        summary:      game.summary,
        cover_url,
        release_year,
        total_rating: game.total_rating,
        platforms,
    }
}

fn unix_seconds_to_year(secs: i64) -> Option<u16> {
    use chrono::{DateTime, Datelike, Utc};
    DateTime::<Utc>::from_timestamp(secs, 0).map(|dt| dt.year() as u16)
}
