pub mod config;
pub mod llm_client;
pub mod provider;
pub mod transcript;
pub mod xdg;

/// RFC 3339 UTC timestamp, e.g. `2026-09-15T12:00:00Z`.
pub fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
