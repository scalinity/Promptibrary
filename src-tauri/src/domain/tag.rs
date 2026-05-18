//! Tag + TagColorSlug per spec §4 "Tag".

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TagColorSlug {
    Gray,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Cyan,
    Purple,
    Pink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub color: TagColorSlug,
    pub count: u32,
    pub last_used_at: Option<DateTime<Utc>>,
}
