//! Secret (API key) Tauri commands — Phase 2.2.
//!
//! The renderer can set, clear, and query *existence* of provider keys. It
//! can never read a stored key back — the backend resolves it at call time
//! (see `services::secrets`).

use serde::Serialize;
use ts_rs::TS;

use crate::error::AppResult;
use crate::services::secrets::{self, SecretProvider};

#[tauri::command]
pub fn secret_set(provider: SecretProvider, value: String) -> AppResult<()> {
    secrets::set(provider, &value)
}

#[tauri::command]
pub fn secret_delete(provider: SecretProvider) -> AppResult<()> {
    secrets::delete(provider)
}

/// Which providers have a key set — booleans only, never the value.
#[tauri::command]
pub fn secret_status() -> Vec<SecretStatus> {
    SecretProvider::all()
        .into_iter()
        .map(|provider| SecretStatus {
            provider,
            present: secrets::has(provider),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/SecretStatus.ts")]
pub struct SecretStatus {
    pub provider: SecretProvider,
    pub present: bool,
}
