use crate::models::NotificationType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: Owner,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Owner {
    pub login: String,
    pub id: u64,
    #[serde(rename = "type")]
    pub owner_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub title: String,
    #[serde(rename = "type")]
    pub subject_type: NotificationType,
    pub url: Option<String>,
    #[serde(rename = "latest_comment_url")]
    pub latest_comment_url: Option<String>,
}
