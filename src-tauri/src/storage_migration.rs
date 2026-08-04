use crate::{
    data_directory::{DataDirectoryMode, DataDirectoryResolver, PendingMigration},
    settings::{DatabaseState, StorageMigrationResult, StorageMigrationStatus, StorageStatus},
};
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageMigrationError {
    #[error("a storage migration is already running")]
    AlreadyRunning,
    #[error("Steam synchronization is running")]
    SyncRunning,
    #[error("the selected storage mode is already active")]
    SameMode,
    #[error("storage migration state is unavailable")]
    StateUnavailable,
    #[error("storage migration failed: {0}")]
    Io(#[from] io::Error),
    #[error("destination database could not be opened: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("storage migration validation failed: {0}")]
    Validation(String),
}

type Manifest = BTreeMap<PathBuf, (PathBuf, u64)>;

pub fn get_storage_status(state: &DatabaseState) -> Result<StorageStatus, StorageMigrationError> {
    let resolver = &state.data_directory;
    let used_bytes = directory_size(resolver.root())?;
    let migration = state
        .storage_migration_status
        .lock()
        .map_err(|_| StorageMigrationError::StateUnavailable)?
        .clone();
    Ok(StorageStatus {
        mode: resolver.mode_name().to_string(),
        current_path: resolver.root().display().to_string(),
        normal_path: resolver.app_data_root().display().to_string(),
        portable_path: resolver.portable_root().display().to_string(),
        used_bytes,
        migration,
    })
}

pub fn migrate_storage(
    state: &DatabaseState,
    target_mode: DataDirectoryMode,
    delete_source: bool,
) -> Result<StorageMigrationResult, StorageMigrationError> {
    if state
        .storage_migration_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(StorageMigrationError::AlreadyRunning);
    }

    if state.steam_sync_running.load(Ordering::SeqCst)
        || state.steam_image_sync_running.load(Ordering::SeqCst)
    {
        state
            .storage_migration_running
            .store(false, Ordering::SeqCst);
        return Err(StorageMigrationError::SyncRunning);
    }

    let resolver = state.data_directory.clone();
    let source_mode = resolver.mode();
    if target_mode == source_mode {
        state
            .storage_migration_running
            .store(false, Ordering::SeqCst);
        return Err(StorageMigrationError::SameMode);
    }

    let target_root = resolver.root_for_mode(target_mode);
    if let Err(error) = set_running_status(state, target_mode, &target_root, delete_source) {
        state
            .storage_migration_running
            .store(false, Ordering::SeqCst);
        return Err(error);
    }

    let result = (|| {
        let connection = state
            .connection
            .lock()
            .map_err(|_| StorageMigrationError::StateUnavailable)?;
        // Finish the WAL before copying so the destination receives a coherent database.
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        migrate_files(&resolver, &connection, target_mode, delete_source, state)
    })();

    state
        .storage_migration_running
        .store(false, Ordering::SeqCst);
    match &result {
        Ok(completed) => set_completed_status(state, completed),
        Err(error) => set_error_status(state, &error.to_string()),
    }
    result
}

fn migrate_files(
    resolver: &DataDirectoryResolver,
    _connection: &Connection,
    target_mode: DataDirectoryMode,
    delete_source: bool,
    state: &DatabaseState,
) -> Result<StorageMigrationResult, StorageMigrationError> {
    let source_root = resolver.root().to_path_buf();
    let target_root = resolver.root_for_mode(target_mode);
    if source_root == target_root {
        return Err(StorageMigrationError::SameMode);
    }

    let target_parent = target_root.parent().ok_or_else(|| {
        StorageMigrationError::Validation("destination has no parent directory".to_string())
    })?;
    fs::create_dir_all(target_parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let staging_root = target_parent.join(format!(".lumadeck-migration-stage-{nonce}"));
    let backup_root = target_parent.join(format!(".lumadeck-migration-backup-{nonce}"));
    if staging_root.exists() || backup_root.exists() {
        return Err(StorageMigrationError::Validation(
            "temporary migration path already exists".to_string(),
        ));
    }

    let migration_result = (|| {
        fs::create_dir_all(&staging_root)?;
        create_storage_directories(&staging_root, target_mode)?;
        let manifest = collect_manifest(&source_root, resolver.mode(), target_mode)?;
        let total_files = manifest.len() as u64;
        let total_bytes = manifest.values().map(|(_, size)| *size).sum();
        update_progress(state, 0, total_files, 0, total_bytes);

        let mut files_copied = 0_u64;
        let mut bytes_copied = 0_u64;
        for (target_relative, (source, expected_size)) in &manifest {
            let destination = staging_root.join(target_relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source, &destination)?;
            files_copied += 1;
            bytes_copied += *expected_size;
            update_progress(state, files_copied, total_files, bytes_copied, total_bytes);
        }

        if target_mode == DataDirectoryMode::Portable {
            DataDirectoryResolver::write_mode_marker(&staging_root, target_mode)?;
        }
        let target_backup = if target_root.exists() {
            Some(backup_root.clone())
        } else {
            None
        };
        DataDirectoryResolver::write_pending_migration(
            &staging_root,
            &PendingMigration {
                source_root: source_root.display().to_string(),
                source_mode: resolver.mode_name().to_string(),
                target_backup: target_backup
                    .as_ref()
                    .map(|path| path.display().to_string()),
                delete_source,
            },
        )?;

        verify_manifest(&staging_root, &manifest)?;
        validate_database(&staging_root.join("lumadeck.db"))?;

        let previous_source_marker = if resolver.mode() == DataDirectoryMode::Portable {
            Some(fs::read(DataDirectoryResolver::storage_mode_marker_path(
                &source_root,
            )))
        } else {
            None
        };

        if let Some(backup) = target_backup.as_ref() {
            fs::rename(&target_root, backup)?;
        }
        if let Err(error) = fs::rename(&staging_root, &target_root) {
            restore_target(&target_root, target_backup.as_deref(), &staging_root)?;
            return Err(StorageMigrationError::Io(error));
        }

        if resolver.mode() == DataDirectoryMode::Portable {
            if let Err(error) =
                DataDirectoryResolver::write_mode_marker(&source_root, DataDirectoryMode::AppData)
            {
                restore_target(&target_root, target_backup.as_deref(), &staging_root)?;
                restore_source_marker(&source_root, previous_source_marker.as_ref())?;
                return Err(StorageMigrationError::Io(error));
            }
        }

        if let Err(error) = validate_database(&target_root.join("lumadeck.db")) {
            restore_target(&target_root, target_backup.as_deref(), &staging_root)?;
            restore_source_marker(&source_root, previous_source_marker.as_ref())?;
            return Err(error);
        }

        Ok(StorageMigrationResult {
            status: "completed".to_string(),
            source_mode: resolver.mode_name().to_string(),
            target_mode: target_mode.as_str().to_string(),
            source_path: source_root.display().to_string(),
            target_path: target_root.display().to_string(),
            files_copied,
            bytes_copied,
            needs_restart: true,
        })
    })();

    if migration_result.is_err() {
        if staging_root.exists() {
            let _ = fs::remove_dir_all(&staging_root);
        }
        if backup_root.exists() && !target_root.exists() {
            let _ = fs::rename(&backup_root, &target_root);
        }
    }
    migration_result
}

fn collect_manifest(
    source_root: &Path,
    source_mode: DataDirectoryMode,
    target_mode: DataDirectoryMode,
) -> Result<Manifest, StorageMigrationError> {
    let mut manifest = BTreeMap::new();
    collect_manifest_recursive(
        source_root,
        source_root,
        source_mode,
        target_mode,
        &mut manifest,
    )?;
    Ok(manifest)
}

fn collect_manifest_recursive(
    source_root: &Path,
    current: &Path,
    source_mode: DataDirectoryMode,
    target_mode: DataDirectoryMode,
    manifest: &mut Manifest,
) -> Result<(), StorageMigrationError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(source_root)
            .map_err(|error| StorageMigrationError::Validation(error.to_string()))?;
        let name = relative.file_name().and_then(|value| value.to_str());
        if name == Some(".lumadeck-storage-mode")
            || name == Some(".lumadeck-migration-pending.json")
            || name.is_some_and(|value| value.starts_with(".lumadeck-migration-"))
        {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_manifest_recursive(source_root, &path, source_mode, target_mode, manifest)?;
        } else if file_type.is_file() {
            let target_relative = map_relative_path(relative, source_mode, target_mode);
            if manifest.contains_key(&target_relative) {
                return Err(StorageMigrationError::Validation(format!(
                    "multiple source files map to {}",
                    target_relative.display()
                )));
            }
            let size = entry.metadata()?.len();
            manifest.insert(target_relative, (path, size));
        } else {
            return Err(StorageMigrationError::Validation(format!(
                "unsupported filesystem entry {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn map_relative_path(
    relative: &Path,
    source_mode: DataDirectoryMode,
    target_mode: DataDirectoryMode,
) -> PathBuf {
    if source_mode == DataDirectoryMode::AppData && target_mode == DataDirectoryMode::Portable {
        if relative == Path::new("settings-runtime.log") {
            return PathBuf::from("logs/settings-runtime.log");
        }
        if let Ok(rest) = relative.strip_prefix("steam-images") {
            return Path::new("assets/steam-images").join(rest);
        }
    }
    if source_mode == DataDirectoryMode::Portable && target_mode == DataDirectoryMode::AppData {
        if relative == Path::new("logs/settings-runtime.log") {
            return PathBuf::from("settings-runtime.log");
        }
        if let Ok(rest) = relative.strip_prefix("assets/steam-images") {
            return Path::new("steam-images").join(rest);
        }
    }
    relative.to_path_buf()
}

fn create_storage_directories(
    root: &Path,
    mode: DataDirectoryMode,
) -> Result<(), StorageMigrationError> {
    fs::create_dir_all(root.join("cache"))?;
    match mode {
        DataDirectoryMode::Portable => {
            fs::create_dir_all(root.join("assets"))?;
            fs::create_dir_all(root.join("logs"))?;
        }
        DataDirectoryMode::AppData => {}
    }
    Ok(())
}

fn verify_manifest(root: &Path, manifest: &Manifest) -> Result<(), StorageMigrationError> {
    for (relative, (_, expected_size)) in manifest {
        let destination = root.join(relative);
        let metadata = fs::metadata(&destination).map_err(|error| {
            StorageMigrationError::Validation(format!(
                "missing copied file {}: {error}",
                destination.display()
            ))
        })?;
        if metadata.len() != *expected_size {
            return Err(StorageMigrationError::Validation(format!(
                "size mismatch for {}: expected {}, got {}",
                relative.display(),
                expected_size,
                metadata.len()
            )));
        }
    }
    Ok(())
}

fn validate_database(path: &Path) -> Result<(), StorageMigrationError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;
    connection.query_row("SELECT 1 FROM sqlite_master LIMIT 1", [], |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(())
}

fn restore_target(
    target_root: &Path,
    backup_root: Option<&Path>,
    staging_root: &Path,
) -> Result<(), StorageMigrationError> {
    if target_root.exists() {
        fs::remove_dir_all(target_root)?;
    }
    if let Some(backup) = backup_root {
        if backup.exists() {
            fs::rename(backup, target_root)?;
        }
    }
    if staging_root.exists() {
        fs::remove_dir_all(staging_root)?;
    }
    Ok(())
}

fn restore_source_marker(
    source_root: &Path,
    previous_marker: Option<&Result<Vec<u8>, io::Error>>,
) -> Result<(), StorageMigrationError> {
    let marker = DataDirectoryResolver::storage_mode_marker_path(source_root);
    match previous_marker {
        Some(Ok(contents)) => fs::write(marker, contents)?,
        Some(Err(_)) | None => {
            if marker.exists() {
                fs::remove_file(marker)?;
            }
        }
    }
    Ok(())
}

fn directory_size(root: &Path) -> Result<u64, StorageMigrationError> {
    let mut total = 0_u64;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total = total.saturating_add(directory_size(&entry.path())?);
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn set_running_status(
    state: &DatabaseState,
    target_mode: DataDirectoryMode,
    target_root: &Path,
    delete_source: bool,
) -> Result<(), StorageMigrationError> {
    let mut status = state
        .storage_migration_status
        .lock()
        .map_err(|_| StorageMigrationError::StateUnavailable)?;
    *status = StorageMigrationStatus {
        status: "running".to_string(),
        current_mode: state.data_directory.mode_name().to_string(),
        current_path: state.data_directory.root().display().to_string(),
        target_mode: Some(target_mode.as_str().to_string()),
        target_path: Some(target_root.display().to_string()),
        files_copied: 0,
        total_files: 0,
        bytes_copied: 0,
        total_bytes: 0,
        error_message: None,
        needs_restart: false,
        delete_source,
    };
    Ok(())
}

fn update_progress(
    state: &DatabaseState,
    files_copied: u64,
    total_files: u64,
    bytes_copied: u64,
    total_bytes: u64,
) {
    if let Ok(mut status) = state.storage_migration_status.lock() {
        status.files_copied = files_copied;
        status.total_files = total_files;
        status.bytes_copied = bytes_copied;
        status.total_bytes = total_bytes;
    }
}

fn set_completed_status(state: &DatabaseState, result: &StorageMigrationResult) {
    if let Ok(mut status) = state.storage_migration_status.lock() {
        status.status = "completed".to_string();
        status.target_mode = Some(result.target_mode.clone());
        status.target_path = Some(result.target_path.clone());
        status.files_copied = result.files_copied;
        status.bytes_copied = result.bytes_copied;
        status.needs_restart = result.needs_restart;
        status.error_message = None;
    }
}

fn set_error_status(state: &DatabaseState, error_message: &str) {
    if let Ok(mut status) = state.storage_migration_status.lock() {
        status.status = "error".to_string();
        status.error_message = Some(error_message.to_string());
        status.needs_restart = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{migrate_storage, DataDirectoryMode, DataDirectoryResolver, DatabaseState};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumadeck-storage-migration-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn open_state(root: &PathBuf, mode: DataDirectoryMode) -> DatabaseState {
        let app_data = root.join("app-data");
        let portable = root.join("data");
        DatabaseState::open(DataDirectoryResolver::for_test_roots(
            &app_data, &portable, mode,
        ))
        .expect("database should open")
    }

    fn seed_current_storage(state: &DatabaseState) {
        let image = state.data_directory.steam_images_directory();
        fs::create_dir_all(&image).expect("image directory");
        fs::write(image.join("cover.webp"), b"webp-data").expect("image");
        let cache = state.data_directory.cache_directory();
        fs::create_dir_all(&cache).expect("cache directory");
        fs::write(cache.join("cache.bin"), b"cache-data").expect("cache");
        let logs = state.data_directory.logs_directory();
        fs::create_dir_all(&logs).expect("log directory");
        fs::write(logs.join("settings-runtime.log"), b"log-data").expect("log");
    }

    #[test]
    fn migrates_normal_to_portable_and_confirms_after_restart() {
        let root = test_root("normal-to-portable");
        let state = open_state(&root, DataDirectoryMode::AppData);
        seed_current_storage(&state);
        let result = migrate_storage(&state, DataDirectoryMode::Portable, false)
            .expect("normal to portable migration");
        assert_eq!(result.target_mode, "portable");
        drop(state);

        let app_data = root.join("app-data");
        let portable = root.join("data");
        let resolver = DataDirectoryResolver::new(root.join("LumaDeck.exe"), &app_data);
        assert_eq!(resolver.mode(), DataDirectoryMode::Portable);
        let restarted = DatabaseState::open(resolver).expect("portable restart");
        assert_eq!(
            fs::read(
                restarted
                    .data_directory
                    .steam_images_directory()
                    .join("cover.webp")
            )
            .expect("portable image"),
            b"webp-data"
        );
        assert!(!DataDirectoryResolver::pending_migration_path(&portable).exists());
        drop(restarted);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_source_as_recovery_when_destination_is_damaged_before_restart() {
        let root = test_root("recovery");
        let state = open_state(&root, DataDirectoryMode::AppData);
        seed_current_storage(&state);
        migrate_storage(&state, DataDirectoryMode::Portable, true)
            .expect("migration should complete");
        drop(state);

        fs::remove_file(root.join("data").join("lumadeck.db")).expect("damage destination");
        let resolver = DataDirectoryResolver::new(root.join("LumaDeck.exe"), root.join("app-data"));
        assert_eq!(resolver.mode(), DataDirectoryMode::Portable);
        let recovered = DatabaseState::open(resolver).expect("source recovery");
        assert_eq!(recovered.data_directory.mode(), DataDirectoryMode::AppData);
        assert!(recovered.data_directory.database_path().is_file());
        assert!(root.join("app-data").is_dir());
        drop(recovered);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migrates_portable_to_normal_and_can_migrate_back() {
        let root = test_root("round-trip");
        let portable = root.join("data");
        fs::create_dir_all(&portable).expect("portable root");
        DataDirectoryResolver::write_mode_marker(&portable, DataDirectoryMode::Portable)
            .expect("portable marker");
        let state = open_state(&root, DataDirectoryMode::Portable);
        seed_current_storage(&state);
        let result = migrate_storage(&state, DataDirectoryMode::AppData, false)
            .expect("portable to normal migration");
        assert_eq!(result.target_mode, "appData");
        drop(state);

        let app_data = root.join("app-data");
        let resolver = DataDirectoryResolver::new(root.join("LumaDeck.exe"), &app_data);
        assert_eq!(resolver.mode(), DataDirectoryMode::AppData);
        let normal_state = DatabaseState::open(resolver).expect("normal restart");
        assert_eq!(
            fs::read(
                normal_state
                    .data_directory
                    .steam_images_directory()
                    .join("cover.webp")
            )
            .expect("normal image"),
            b"webp-data"
        );
        let result = migrate_storage(&normal_state, DataDirectoryMode::Portable, false)
            .expect("second migration");
        assert_eq!(result.target_mode, "portable");
        drop(normal_state);

        let resolver = DataDirectoryResolver::new(root.join("LumaDeck.exe"), &app_data);
        assert_eq!(resolver.mode(), DataDirectoryMode::Portable);
        let final_state = DatabaseState::open(resolver).expect("second restart");
        assert!(final_state.data_directory.database_path().is_file());
        drop(final_state);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mapping_conflict_keeps_source_and_does_not_activate_destination() {
        let root = test_root("conflict");
        let state = open_state(&root, DataDirectoryMode::AppData);
        seed_current_storage(&state);
        fs::create_dir_all(state.data_directory.root().join("logs")).expect("logs");
        fs::write(
            state
                .data_directory
                .root()
                .join("logs")
                .join("settings-runtime.log"),
            b"different-log",
        )
        .expect("conflicting log");
        let error = migrate_storage(&state, DataDirectoryMode::Portable, false)
            .expect_err("mapping conflict should fail");
        assert!(error.to_string().contains("multiple source files"));
        assert!(state.data_directory.database_path().is_file());
        assert!(!root.join("data").join("lumadeck.db").exists());
        drop(state);
        let _ = fs::remove_dir_all(root);
    }
}
