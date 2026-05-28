//! `.verbatim` project file format — Phase 1.1.
//!
//! A `.verbatim` file is a SQLite database. We chose SQLite over a flat
//! JSON file for three reasons:
//!   1. Atomic, crash-safe writes (WAL) — a power-cut mid-save can't
//!      corrupt a project.
//!   2. A `meta` table carries a schema version, so v2 can migrate v1
//!      files forward.
//!   3. The `caption` table is normalized (one row per caption), leaving
//!      room to query/stream captions for very large projects without
//!      parsing a multi-MB JSON blob. (For v1 the renderer still loads
//!      the whole project into memory; the structure is forward-looking.)
//!
//! Scalar Project fields + style/speakers/glossary live in a single
//! `project` row (the collections as JSON — they're small). Captions get
//! their own table; each caption's `words` are JSON (words are an
//! implementation detail of a caption, always loaded together).

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

use crate::error::{AppError, AppResult};
use crate::model::{Caption, GlossaryTerm, Project, Speaker, Style, Word};

const SCHEMA_VERSION: i64 = 1;

/// Open (creating if missing) a `.verbatim` SQLite file and ensure schema.
async fn open_pool(path: &Path) -> AppResult<SqlitePool> {
    let url = format!("sqlite:{}", path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}

async fn ensure_schema(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    ).execute(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS project (
            id                  TEXT PRIMARY KEY,
            name                TEXT NOT NULL,
            video_path          TEXT NOT NULL,
            video_content_hash  TEXT NOT NULL,
            video_duration_ms   INTEGER NOT NULL,
            video_width         INTEGER NOT NULL,
            video_height        INTEGER NOT NULL,
            video_fps           REAL NOT NULL,
            audio_wav_path      TEXT,
            language            TEXT NOT NULL,
            default_style_json  TEXT NOT NULL,
            context_description TEXT,
            speakers_json       TEXT NOT NULL,
            glossary_json       TEXT NOT NULL,
            created_at          INTEGER NOT NULL,
            updated_at          INTEGER NOT NULL
        );
        "#,
    ).execute(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS caption (
            id             TEXT PRIMARY KEY,
            position       INTEGER NOT NULL,
            start_ms       INTEGER NOT NULL,
            end_ms         INTEGER NOT NULL,
            words_json     TEXT NOT NULL,
            speaker_id     TEXT,
            style_id       TEXT,
            notes          TEXT,
            ai_generated   INTEGER NOT NULL,
            last_edited_at INTEGER NOT NULL
        );
        "#,
    ).execute(pool).await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS caption_pos_idx ON caption(position)")
        .execute(pool).await?;

    sqlx::query("INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', ?1)")
        .bind(SCHEMA_VERSION.to_string())
        .execute(pool).await?;

    Ok(())
}

/// Save a project to `path`, replacing any existing content. Atomic at
/// the row level via a transaction.
pub async fn save(project: &Project, path: &Path) -> AppResult<()> {
    let pool = open_pool(path).await?;
    let mut tx = pool.begin().await?;

    // Replace project row
    sqlx::query("DELETE FROM project").execute(&mut *tx).await?;
    sqlx::query(
        r#"
        INSERT INTO project (id, name, video_path, video_content_hash,
            video_duration_ms, video_width, video_height, video_fps,
            audio_wav_path, language, default_style_json, context_description,
            speakers_json, glossary_json, created_at, updated_at)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
        "#,
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.video_path)
    .bind(&project.video_content_hash)
    .bind(project.video_duration_ms)
    .bind(project.video_width)
    .bind(project.video_height)
    .bind(project.video_fps)
    .bind(&project.audio_wav_path)
    .bind(&project.language)
    .bind(serde_json::to_string(&project.default_style)?)
    .bind(&project.context_description)
    .bind(serde_json::to_string(&project.speakers)?)
    .bind(serde_json::to_string(&project.glossary)?)
    .bind(project.created_at)
    .bind(project.updated_at)
    .execute(&mut *tx)
    .await?;

    // Replace captions
    sqlx::query("DELETE FROM caption").execute(&mut *tx).await?;
    for (pos, c) in project.captions.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO caption (id, position, start_ms, end_ms, words_json,
                speaker_id, style_id, notes, ai_generated, last_edited_at)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
            "#,
        )
        .bind(&c.id)
        .bind(pos as i64)
        .bind(c.start_ms)
        .bind(c.end_ms)
        .bind(serde_json::to_string(&c.words)?)
        .bind(&c.speaker_id)
        .bind(&c.style_id)
        .bind(&c.notes)
        .bind(if c.ai_generated { 1 } else { 0 })
        .bind(c.last_edited_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    pool.close().await;
    Ok(())
}

/// Load a project from `path`.
pub async fn load(path: &Path) -> AppResult<Project> {
    if !path.exists() {
        return Err(AppError::NotFound {
            entity: "project_file",
            id: path.to_string_lossy().to_string(),
        });
    }
    let pool = open_pool(path).await?;

    let row = sqlx::query("SELECT * FROM project LIMIT 1")
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::Validation("project file has no project row".to_string()))?;

    let default_style: Style = serde_json::from_str(row.get::<String, _>("default_style_json").as_str())?;
    let speakers: Vec<Speaker> = serde_json::from_str(row.get::<String, _>("speakers_json").as_str())?;
    let glossary: Vec<GlossaryTerm> = serde_json::from_str(row.get::<String, _>("glossary_json").as_str())?;

    let caption_rows = sqlx::query("SELECT * FROM caption ORDER BY position")
        .fetch_all(&pool)
        .await?;

    let mut captions = Vec::with_capacity(caption_rows.len());
    for r in caption_rows {
        let words: Vec<Word> = serde_json::from_str(r.get::<String, _>("words_json").as_str())?;
        captions.push(Caption {
            id: r.get("id"),
            start_ms: r.get("start_ms"),
            end_ms: r.get("end_ms"),
            words,
            speaker_id: r.get("speaker_id"),
            style_id: r.get("style_id"),
            notes: r.get("notes"),
            ai_generated: r.get::<i64, _>("ai_generated") != 0,
            last_edited_at: r.get("last_edited_at"),
        });
    }

    let project = Project {
        id: row.get("id"),
        name: row.get("name"),
        video_path: row.get("video_path"),
        video_content_hash: row.get("video_content_hash"),
        video_duration_ms: row.get("video_duration_ms"),
        video_width: row.get::<i64, _>("video_width") as i32,
        video_height: row.get::<i64, _>("video_height") as i32,
        video_fps: row.get::<f64, _>("video_fps") as f32,
        audio_wav_path: row.get("audio_wav_path"),
        language: row.get("language"),
        default_style,
        context_description: row.get("context_description"),
        captions,
        speakers,
        glossary,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    pool.close().await;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Word;

    fn sample_project() -> Project {
        Project {
            id: "p1".into(),
            name: "test.mp4".into(),
            video_path: "/videos/test.mp4".into(),
            video_content_hash: "abc123".into(),
            video_duration_ms: 90_000,
            video_width: 1920,
            video_height: 1080,
            video_fps: 29.97,
            audio_wav_path: Some("/cache/test.wav".into()),
            language: "no".into(),
            default_style: Style::broadcast_news(),
            context_description: Some("A sermon about grace.".into()),
            captions: vec![
                Caption {
                    id: "c1".into(), start_ms: 0, end_ms: 2000,
                    words: vec![
                        Word::new("Hello", 0, 500, 95.0),
                        Word::new("world", 500, 2000, 72.0),
                    ],
                    speaker_id: Some("s1".into()),
                    style_id: None, notes: Some("note".into()),
                    ai_generated: true, last_edited_at: 100,
                },
                Caption {
                    id: "c2".into(), start_ms: 2500, end_ms: 4000,
                    words: vec![Word::new("Again", 2500, 4000, 88.0)],
                    speaker_id: None, style_id: None, notes: None,
                    ai_generated: false, last_edited_at: 200,
                },
            ],
            speakers: vec![
                Speaker { id: "s1".into(), display_name: "Lars".into(), color_hex: Some("#4FD1C5".into()) },
            ],
            glossary: vec![
                GlossaryTerm { id: "g1".into(), term: "kerygma".into(),
                    aliases: vec!["kerigma".into()], definition: None, pronunciation_hint: None },
            ],
            created_at: 1000,
            updated_at: 2000,
        }
    }

    #[tokio::test]
    async fn save_and_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.verbatim");
        let original = sample_project();

        save(&original, &path).await.unwrap();
        let loaded = load(&path).await.unwrap();

        assert_eq!(loaded, original, "round-trip must preserve the project exactly");
    }

    #[tokio::test]
    async fn save_overwrites_previous_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.verbatim");

        let mut p = sample_project();
        save(&p, &path).await.unwrap();

        // Remove a caption and re-save — the old caption must not linger
        p.captions.pop();
        p.updated_at = 9999;
        save(&p, &path).await.unwrap();

        let loaded = load(&path).await.unwrap();
        assert_eq!(loaded.captions.len(), 1);
        assert_eq!(loaded.updated_at, 9999);
    }

    #[tokio::test]
    async fn load_missing_file_errors() {
        let err = load(Path::new("/nonexistent/x.verbatim")).await.unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[tokio::test]
    async fn caption_order_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("order.verbatim");
        let p = sample_project();
        save(&p, &path).await.unwrap();
        let loaded = load(&path).await.unwrap();
        assert_eq!(loaded.captions[0].id, "c1");
        assert_eq!(loaded.captions[1].id, "c2");
    }
}
