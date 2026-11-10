//! Atomic backup and time machine snapshot engine.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub id: String,
    pub timestamp: String,
    pub tag: String,
    pub description: String,
    pub backed_up_files: Vec<String>,
    pub snapshot_dir: PathBuf,
}

pub struct BackupEngine;

impl BackupEngine {
    /// Create a snapshot of specified files under `.devcraft/snapshots/<timestamp>_<tag>`.
    pub fn create_snapshot(
        root_dir: &Path,
        files: &[PathBuf],
        tag: &str,
        description: &str,
    ) -> Result<SnapshotRecord, String> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let clean_tag = if tag.trim().is_empty() {
            "checkpoint"
        } else {
            tag.trim()
        };
        let snapshot_id = format!("{}_{}", timestamp, clean_tag);
        let snapshot_dir = root_dir.join(".devcraft").join("snapshots").join(&snapshot_id);

        if let Err(e) = fs::create_dir_all(&snapshot_dir) {
            return Err(format!("Failed to create snapshot dir {:?}: {}", snapshot_dir, e));
        }

        let mut backed_up = Vec::new();

        for file in files {
            if !file.exists() || !file.is_file() {
                continue;
            }

            let file_name = match file.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };

            let dest = snapshot_dir.join(&file_name);
            if let Err(e) = fs::copy(file, &dest) {
                return Err(format!("Failed to copy file {:?} to {:?}: {}", file, dest, e));
            }

            backed_up.push(file.to_string_lossy().to_string());
        }

        let record = SnapshotRecord {
            id: snapshot_id,
            timestamp,
            tag: clean_tag.to_string(),
            description: description.to_string(),
            backed_up_files: backed_up,
            snapshot_dir: snapshot_dir.clone(),
        };

        // Write metadata json inside snapshot dir
        let meta_path = snapshot_dir.join("metadata.json");
        if let Ok(json_bytes) = serde_json::to_vec_pretty(&record) {
            let _ = fs::write(meta_path, json_bytes);
        }

        Ok(record)
    }

    /// List existing snapshots in the repository or fleet root.
    pub fn list_snapshots(root_dir: &Path) -> Vec<SnapshotRecord> {
        let snapshots_dir = root_dir.join(".devcraft").join("snapshots");
        let mut list = Vec::new();

        if let Ok(entries) = fs::read_dir(snapshots_dir) {
            for entry in entries.flatten() {
                let meta_file = entry.path().join("metadata.json");
                if meta_file.exists() {
                    if let Ok(bytes) = fs::read(&meta_file) {
                        if let Ok(record) = serde_json::from_slice::<SnapshotRecord>(&bytes) {
                            list.push(record);
                        }
                    }
                }
            }
        }

        list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        list
    }
}
