//! Project lifecycle Tauri commands — Phase 1.
//!
//! probe a video, create a project from it, open/save `.verbatim` files,
//! compute the waveform, and relink a moved video.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::model::{Project, Style};
use crate::services::{project_file, video, waveform};
use crate::services::video::VideoMetadata;
use crate::services::waveform::WaveformData;

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Probe a media file's metadata (ffprobe).
#[tauri::command]
pub fn video_probe(path: String) -> AppResult<VideoMetadata> {
    video::probe(Path::new(&path))
}

/// Create a fresh in-memory Project from a video file. Captions are empty
/// until the user transcribes (Phase 2). The video is hashed for path
/// stability and a sensible default style is applied.
#[tauri::command]
pub fn project_create_from_video(path: String) -> AppResult<Project> {
    let p = Path::new(&path);
    let meta = video::probe(p)?;
    let hash = video::content_hash(p)?;
    let name = p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let now = now_ms();

    Ok(Project {
        id: new_id(),
        name,
        video_path: path.clone(),
        video_content_hash: hash,
        video_duration_ms: meta.duration_ms,
        video_width: meta.width,
        video_height: meta.height,
        video_fps: meta.fps,
        audio_wav_path: None, // set after extraction
        language: "auto".into(),
        default_style: Style::broadcast_news(),
        context_description: None,
        captions: vec![],
        speakers: vec![],
        glossary: vec![],
        created_at: now,
        updated_at: now,
    })
}

#[tauri::command]
pub async fn project_save(project: Project, path: String) -> AppResult<()> {
    project_file::save(&project, Path::new(&path)).await
}

#[tauri::command]
pub async fn project_open(path: String) -> AppResult<Project> {
    let project = project_file::load(Path::new(&path)).await?;
    // Verify the source video still exists; if not, the renderer shows
    // the relink UI keyed off the `video_missing` error.
    if !Path::new(&project.video_path).exists() {
        return Err(AppError::VideoMissing(project.video_path.clone()));
    }
    Ok(project)
}

/// Extract audio + compute multi-zoom waveform peaks for a video.
/// Writes the WAV to `cache_dir` (typically the app's project cache).
#[tauri::command]
pub fn waveform_compute(video_path: String, cache_dir: String) -> AppResult<WaveformData> {
    let input = Path::new(&video_path);
    let wav = Path::new(&cache_dir).join(format!(
        "{}.wav",
        video::content_hash(input)?,
    ));
    if !wav.exists() {
        waveform::extract_audio_wav(input, &wav)?;
    }
    let (samples, sample_rate) = waveform::read_wav_samples(&wav)?;
    // 800 base buckets ≈ a typical editor width; ×4 per finer level, 5 levels.
    Ok(waveform::compute_levels(&samples, sample_rate, 800, 4, 5))
}

/// Try to relink a project whose video moved. Searches the provided dirs.
#[tauri::command]
pub fn project_relink(
    target_hash: String,
    search_dirs: Vec<String>,
    original_filename: Option<String>,
) -> AppResult<Option<String>> {
    let dirs: Vec<PathBuf> = search_dirs.into_iter().map(PathBuf::from).collect();
    let found = video::find_relink_candidate(&target_hash, &dirs, original_filename.as_deref())?;
    Ok(found.map(|p| p.to_string_lossy().to_string()))
}

/// The accepted file extensions — used by the renderer to build the
/// native open-file dialog filter.
#[tauri::command]
pub fn accepted_media_extensions() -> Vec<String> {
    video::accepted_extensions().into_iter().map(String::from).collect()
}
