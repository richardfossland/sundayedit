//! Secure API-key storage — Phase 2.2.
//!
//! Keys live in the OS keychain (Keychain on macOS, Credential Manager on
//! Windows) via the `keyring` crate — NEVER in plaintext files, and NEVER
//! returned to the renderer. The UI can set, clear, and check existence; the
//! backend reads the secret only at call time via `resolve`.

use keyring::Entry;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "app.sundayedit";

/// Providers whose API keys we store. Anthropic powers the AI features today;
/// the cloud-ASR providers are here so the same store is ready when their
/// transcription path is wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../src/lib/bindings/SecretProvider.ts")]
pub enum SecretProvider {
    Anthropic,
    OpenAi,
    AssemblyAi,
    Deepgram,
}

impl SecretProvider {
    /// Stable keychain account name — must never change or stored keys are
    /// orphaned.
    fn account(self) -> &'static str {
        match self {
            SecretProvider::Anthropic => "anthropic",
            SecretProvider::OpenAi => "openai",
            SecretProvider::AssemblyAi => "assemblyai",
            SecretProvider::Deepgram => "deepgram",
        }
    }

    pub fn all() -> [SecretProvider; 4] {
        [
            SecretProvider::Anthropic,
            SecretProvider::OpenAi,
            SecretProvider::AssemblyAi,
            SecretProvider::Deepgram,
        ]
    }
}

fn entry(provider: SecretProvider) -> AppResult<Entry> {
    Entry::new(SERVICE, provider.account())
        .map_err(|e| AppError::Internal(format!("keychain: {e}")))
}

/// Store (or overwrite) a provider's key.
pub fn set(provider: SecretProvider, value: &str) -> AppResult<()> {
    entry(provider)?
        .set_password(value)
        .map_err(|e| AppError::Internal(format!("keychain set: {e}")))
}

/// Remove a provider's key. Idempotent — clearing an absent key is fine.
pub fn delete(provider: SecretProvider) -> AppResult<()> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Internal(format!("keychain delete: {e}"))),
    }
}

/// The stored secret, or None. Backend-only — never hand this to the renderer.
pub fn get(provider: SecretProvider) -> Option<String> {
    entry(provider).ok()?.get_password().ok()
}

/// Whether a key is set — the only thing the UI is told.
pub fn has(provider: SecretProvider) -> bool {
    get(provider).is_some()
}

/// Resolve a key for a call: an explicit per-call key wins, then the keychain,
/// then the env var (CI / power users), else empty.
pub fn resolve(explicit: Option<String>, provider: SecretProvider, env_var: &str) -> String {
    resolve_from(explicit, get(provider), std::env::var(env_var).ok())
}

/// Pure precedence logic, split out so it's testable without the OS keychain.
fn resolve_from(explicit: Option<String>, keychain: Option<String>, env: Option<String>) -> String {
    explicit
        .filter(|k| !k.trim().is_empty())
        .or(keychain)
        .or(env)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_key_wins() {
        let out = resolve_from(
            Some("explicit".into()),
            Some("keychain".into()),
            Some("env".into()),
        );
        assert_eq!(out, "explicit");
    }

    #[test]
    fn blank_explicit_falls_through_to_keychain() {
        let out = resolve_from(
            Some("   ".into()),
            Some("keychain".into()),
            Some("env".into()),
        );
        assert_eq!(out, "keychain");
    }

    #[test]
    fn keychain_beats_env() {
        let out = resolve_from(None, Some("keychain".into()), Some("env".into()));
        assert_eq!(out, "keychain");
    }

    #[test]
    fn env_is_last_resort() {
        let out = resolve_from(None, None, Some("env".into()));
        assert_eq!(out, "env");
    }

    #[test]
    fn nothing_set_yields_empty() {
        assert_eq!(resolve_from(None, None, None), "");
    }

    #[test]
    fn provider_accounts_are_distinct() {
        let accounts: std::collections::HashSet<_> =
            SecretProvider::all().iter().map(|p| p.account()).collect();
        assert_eq!(accounts.len(), 4);
    }
}
