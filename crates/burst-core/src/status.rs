//! Custom status: a short text and an emoji a person shows beside their name,
//! optionally until a set time.

use chrono::{DateTime, Utc};

/// Longest status text, in characters.
pub const MAX_TEXT_CHARS: usize = 100;
/// Longest status emoji, in characters. Room for ZWJ sequences and flags.
pub const MAX_EMOJI_CHARS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomStatus {
    pub text: Option<String>,
    pub emoji: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StatusError {
    #[error("a status needs a text or an emoji")]
    Empty,
    #[error("status text is longer than {MAX_TEXT_CHARS} characters")]
    TextTooLong,
    #[error("status emoji must be a single emoji")]
    InvalidEmoji,
    #[error("status expiry is in the past")]
    AlreadyExpired,
}

impl CustomStatus {
    /// Trims and checks a status someone is setting. Blank parts are dropped.
    pub fn new(
        text: Option<&str>,
        emoji: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<Self, StatusError> {
        let text = text.map(str::trim).filter(|t| !t.is_empty());
        let emoji = emoji.map(str::trim).filter(|e| !e.is_empty());
        if text.is_none() && emoji.is_none() {
            return Err(StatusError::Empty);
        }
        if text.is_some_and(|t| t.chars().count() > MAX_TEXT_CHARS) {
            return Err(StatusError::TextTooLong);
        }
        if emoji.is_some_and(|e| !is_emoji_like(e)) {
            return Err(StatusError::InvalidEmoji);
        }
        if expires_at.is_some_and(|at| at <= now) {
            return Err(StatusError::AlreadyExpired);
        }
        Ok(Self {
            text: text.map(String::from),
            emoji: emoji.map(String::from),
            expires_at,
        })
    }
}

/// Whether a status with this expiry is still shown at `now`.
pub fn is_active(expires_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    expires_at.is_none_or(|at| at > now)
}

/// Short, no whitespace or control characters, and not plain ASCII text.
fn is_emoji_like(s: &str) -> bool {
    s.chars().count() <= MAX_EMOJI_CHARS
        && !s.chars().any(|c| c.is_whitespace() || c.is_control())
        && !s.is_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        "2026-09-24T12:00:00Z".parse().unwrap()
    }

    #[test]
    fn keeps_text_and_emoji_trimmed() {
        let s = CustomStatus::new(Some("  In a meeting "), Some(" 📅 "), None, now()).unwrap();
        assert_eq!(s.text.as_deref(), Some("In a meeting"));
        assert_eq!(s.emoji.as_deref(), Some("📅"));
    }

    #[test]
    fn either_part_alone_is_enough() {
        assert!(CustomStatus::new(Some("Lunch"), None, None, now()).is_ok());
        assert!(CustomStatus::new(None, Some("🍕"), None, now()).is_ok());
    }

    #[test]
    fn a_blank_status_is_refused() {
        assert_eq!(
            CustomStatus::new(None, None, None, now()),
            Err(StatusError::Empty)
        );
        assert_eq!(
            CustomStatus::new(Some("  "), Some(""), None, now()),
            Err(StatusError::Empty)
        );
    }

    #[test]
    fn text_is_limited_in_characters_not_bytes() {
        let at_limit = "é".repeat(MAX_TEXT_CHARS);
        assert!(CustomStatus::new(Some(&at_limit), None, None, now()).is_ok());
        let over = "é".repeat(MAX_TEXT_CHARS + 1);
        assert_eq!(
            CustomStatus::new(Some(&over), None, None, now()),
            Err(StatusError::TextTooLong)
        );
    }

    #[test]
    fn emoji_sequences_are_accepted() {
        for e in ["👩‍💻", "🇫🇷", "👍🏽", "❤️"] {
            assert!(CustomStatus::new(None, Some(e), None, now()).is_ok(), "{e}");
        }
    }

    #[test]
    fn words_are_not_an_emoji() {
        for e in [
            "busy",
            ":tada:",
            "🎉 party",
            "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉",
        ] {
            assert_eq!(
                CustomStatus::new(None, Some(e), None, now()),
                Err(StatusError::InvalidEmoji),
                "{e}"
            );
        }
    }

    #[test]
    fn an_expiry_must_be_in_the_future() {
        let s = CustomStatus::new(Some("x"), None, Some(now() + Duration::hours(1)), now());
        assert!(s.is_ok());
        for at in [now(), now() - Duration::seconds(1)] {
            assert_eq!(
                CustomStatus::new(Some("x"), None, Some(at), now()),
                Err(StatusError::AlreadyExpired)
            );
        }
    }

    #[test]
    fn a_status_is_shown_until_it_expires() {
        assert!(is_active(None, now()));
        assert!(is_active(Some(now() + Duration::seconds(1)), now()));
        assert!(!is_active(Some(now()), now()));
        assert!(!is_active(Some(now() - Duration::seconds(1)), now()));
    }
}
