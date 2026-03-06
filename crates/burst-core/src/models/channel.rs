use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: Uuid,
    pub kind: ChannelKind,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub is_archived: bool,
    pub is_readonly: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Public,
    Private,
    Dm,
    GroupDm,
}

impl ChannelKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Dm => "dm",
            Self::GroupDm => "group_dm",
        }
    }
}

impl std::str::FromStr for ChannelKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Self::Public),
            "private" => Ok(Self::Private),
            "dm" => Ok(Self::Dm),
            "group_dm" => Ok(Self::GroupDm),
            _ => Err(format!("unknown channel kind: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelMemberRole {
    Owner,
    Moderator,
    Member,
}

impl ChannelMemberRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Moderator => "moderator",
            Self::Member => "member",
        }
    }
}

impl std::str::FromStr for ChannelMemberRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "moderator" => Ok(Self::Moderator),
            "member" => Ok(Self::Member),
            _ => Err(format!("unknown channel member role: {s}")),
        }
    }
}
