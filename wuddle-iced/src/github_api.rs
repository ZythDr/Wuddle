//! Shared GitHub API response handling for frontend-owned requests.
//!
//! Keep rate-limit detection here so README previews, Quick Add, release notes,
//! curated patches, and self-update checks all explain the same failure.

use reqwest::{Response, StatusCode};
use std::time::{SystemTime, UNIX_EPOCH};

const RATE_LIMIT_PREFIX: &str = "GITHUB_RATE_LIMIT:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitNotice {
    pub message: String,
    pub reset_epoch: Option<i64>,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn reset_description(reset_epoch: Option<i64>) -> String {
    let Some(reset_epoch) = reset_epoch else {
        return "when GitHub resets the hourly quota".to_string();
    };
    let seconds = (reset_epoch - now_unix()).max(0);
    let minutes = ((seconds + 59) / 60).max(1);
    format!(
        "in about {minutes} minute{}",
        if minutes == 1 { "" } else { "s" }
    )
}

fn encoded_rate_limit_error(has_token: bool, reset_epoch: Option<i64>) -> String {
    let reset = reset_epoch
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let reset_text = reset_description(reset_epoch);
    let explanation = if has_token {
        format!(
            "GitHub's API limit has been reached. Requests should work again {reset_text}. The saved token may be expired, invalid, or shared with other applications; re-save it in Options."
        )
    } else {
        format!(
            "GitHub's anonymous API limit of 60 requests per hour has been reached. Requests should work again {reset_text}. Add a GitHub token in Options to raise the limit to 5,000 requests per hour."
        )
    };
    format!("{RATE_LIMIT_PREFIX}{reset}:{explanation}")
}

/// Convert a non-successful GitHub response into a safe, actionable error.
pub async fn checked_response(response: Response) -> Result<Response, String> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let remaining_is_zero = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "0");
    let reset_epoch = response
        .headers()
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok());
    let body = response.text().await.unwrap_or_default();
    let body_lower = body.to_ascii_lowercase();
    let rate_limited = status == StatusCode::TOO_MANY_REQUESTS
        || remaining_is_zero
        || ((status == StatusCode::FORBIDDEN) && body_lower.contains("rate limit"));
    let has_token = wuddle_engine::github_token().is_some();

    if rate_limited {
        return Err(encoded_rate_limit_error(has_token, reset_epoch));
    }
    if status == StatusCode::UNAUTHORIZED
        || body_lower.contains("bad credentials")
        || body_lower.contains("requires authentication")
    {
        return Err(if has_token {
            "GitHub authentication failed. Re-save or replace the saved token in Options."
                .to_string()
        } else {
            "GitHub requires authentication for this request. Add a GitHub token in Options."
                .to_string()
        });
    }
    if status == StatusCode::NOT_FOUND {
        return Err("GitHub could not find this repository or resource (HTTP 404).".to_string());
    }
    Err(format!(
        "GitHub could not complete the request (HTTP {}).",
        status.as_u16()
    ))
}

/// Extract a rate-limit notice even when another operation wrapped the error.
pub fn rate_limit_notice(error: &str) -> Option<RateLimitNotice> {
    let encoded = error.split_once(RATE_LIMIT_PREFIX)?.1;
    let (reset, message) = encoded.split_once(':')?;
    let reset_epoch = reset.parse::<i64>().ok();
    let message = if reset_epoch.is_some() && !message.contains("Requests should work again") {
        format!(
            "{} Requests should work again {}.",
            message.trim(),
            reset_description(reset_epoch)
        )
    } else {
        message.trim().to_string()
    };
    Some(RateLimitNotice {
        message,
        reset_epoch,
    })
}

pub fn user_facing_error(error: &str) -> String {
    rate_limit_notice(error)
        .map(|notice| notice.message)
        .unwrap_or_else(|| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_rate_limit_errors_remain_classifiable() {
        let error = format!(
            "README failed: {}",
            encoded_rate_limit_error(false, Some(now_unix() + 120))
        );
        let notice = rate_limit_notice(&error).expect("rate-limit marker");
        assert!(notice.message.contains("60 requests per hour"));
        assert!(notice.message.contains("5,000 requests per hour"));
        assert!(notice.reset_epoch.is_some());
    }

    #[test]
    fn ordinary_errors_are_not_rate_limits() {
        assert!(rate_limit_notice("GitHub returned HTTP 404").is_none());
    }
}
