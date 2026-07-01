use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static EXPORT_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_id() -> u64 {
    EXPORT_COUNTER.fetch_add(1, Ordering::SeqCst)
}

pub fn next_editor_export_path(extension: &str) -> Result<PathBuf, String> {
    let clean_extension = extension
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    let extension = if clean_extension.is_empty() {
        "ply".to_string()
    } else {
        clean_extension
    };
    let dir = std::env::temp_dir()
        .join("augmented-gaussian")
        .join("editor-export");
    std::fs::create_dir_all(&dir).map_err(|err| {
        format!(
            "failed to create editor export directory '{}': {}",
            dir.to_string_lossy(),
            err
        )
    })?;
    Ok(dir.join(format!(
        "edited-{}-{}.{extension}",
        std::process::id(),
        next_id()
    )))
}
