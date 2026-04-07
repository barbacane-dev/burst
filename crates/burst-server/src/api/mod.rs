pub mod admin;
pub mod attachments;
pub mod channels;
pub mod extractors;
pub mod health;
pub mod search;
pub mod users;
pub mod webhooks;

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
