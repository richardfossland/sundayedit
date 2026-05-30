//! Sunday-link deep-link import commands — Phase 8.
//!
//! Thin layer over `services::deeplink`. The native deep-link plugin (wired in
//! `lib.rs`) emits the raw URL to the renderer; the renderer calls
//! `deeplink_parse_import` to validate + structure it, then drives the normal
//! import + context/glossary seeding flow itself.

use crate::error::AppResult;
use crate::services::deeplink::{parse_import_url, ImportRequest};

/// Parse a `sundayedit://import?…` URL into a validated [`ImportRequest`].
#[tauri::command]
pub fn deeplink_parse_import(url: String) -> AppResult<ImportRequest> {
    parse_import_url(&url)
}
