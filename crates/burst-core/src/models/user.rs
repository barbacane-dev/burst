use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub external_id: Option<String>,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub role: Role,
    pub status: UserStatus,
    pub status_text: Option<String>,
    pub is_bot: bool,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Moderator,
    Member,
    Guest,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Moderator => "moderator",
            Self::Member => "member",
            Self::Guest => "guest",
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "moderator" => Ok(Self::Moderator),
            "member" => Ok(Self::Member),
            "guest" => Ok(Self::Guest),
            _ => Err(format!("unknown role: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Online,
    Away,
    Offline,
    Dnd,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Away => "away",
            Self::Offline => "offline",
            Self::Dnd => "dnd",
        }
    }
}

impl std::str::FromStr for UserStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "online" => Ok(Self::Online),
            "away" => Ok(Self::Away),
            "offline" => Ok(Self::Offline),
            "dnd" => Ok(Self::Dnd),
            _ => Err(format!("unknown status: {s}")),
        }
    }
}
