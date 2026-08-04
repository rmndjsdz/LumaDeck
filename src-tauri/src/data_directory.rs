use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const STORAGE_MODE_MARKER: &str = ".lumadeck-storage-mode";
const PENDING_MIGRATION_MARKER: &str = ".lumadeck-migration-pending.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDirectoryMode {
    AppData,
    Portable,
}

#[derive(Debug, Clone)]
pub struct DataDirectoryResolver {
    root: PathBuf,
    mode: DataDirectoryMode,
    app_data_root: PathBuf,
    portable_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingMigration {
    pub source_root: String,
    pub source_mode: String,
    pub target_backup: Option<String>,
    pub delete_source: bool,
}

impl DataDirectoryResolver {
    pub fn new(executable_path: impl AsRef<Path>, app_data_dir: impl AsRef<Path>) -> Self {
        let executable_path = executable_path.as_ref();
        let app_data_dir = app_data_dir.as_ref();
        let portable_root = executable_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("data");

        let portable_disabled = fs::read_to_string(portable_root.join(STORAGE_MODE_MARKER))
            .map(|value| value.trim().eq_ignore_ascii_case("appdata"))
            .unwrap_or(false);

        if portable_root.is_dir() && !portable_disabled {
            Self {
                root: portable_root,
                mode: DataDirectoryMode::Portable,
                app_data_root: app_data_dir.to_path_buf(),
                portable_root: executable_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("data"),
            }
        } else {
            Self {
                root: app_data_dir.to_path_buf(),
                mode: DataDirectoryMode::AppData,
                app_data_root: app_data_dir.to_path_buf(),
                portable_root,
            }
        }
    }

    #[cfg(test)]
    pub fn for_app_data(app_data_dir: impl AsRef<Path>) -> Self {
        Self {
            root: app_data_dir.as_ref().to_path_buf(),
            mode: DataDirectoryMode::AppData,
            app_data_root: app_data_dir.as_ref().to_path_buf(),
            portable_root: PathBuf::new(),
        }
    }

    #[cfg(test)]
    pub fn for_test_roots(
        app_data_root: impl AsRef<Path>,
        portable_root: impl AsRef<Path>,
        mode: DataDirectoryMode,
    ) -> Self {
        let app_data_root = app_data_root.as_ref().to_path_buf();
        let portable_root = portable_root.as_ref().to_path_buf();
        Self::from_roots(
            match mode {
                DataDirectoryMode::AppData => app_data_root.clone(),
                DataDirectoryMode::Portable => portable_root.clone(),
            },
            mode,
            app_data_root,
            portable_root,
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn mode(&self) -> DataDirectoryMode {
        self.mode
    }

    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            DataDirectoryMode::AppData => "appData",
            DataDirectoryMode::Portable => "portable",
        }
    }

    pub fn app_data_root(&self) -> &Path {
        &self.app_data_root
    }

    pub fn portable_root(&self) -> &Path {
        &self.portable_root
    }

    pub fn root_for_mode(&self, mode: DataDirectoryMode) -> PathBuf {
        match mode {
            DataDirectoryMode::AppData => self.app_data_root.clone(),
            DataDirectoryMode::Portable => self.portable_root.clone(),
        }
    }

    pub fn database_path(&self) -> PathBuf {
        self.root.join("lumadeck.db")
    }

    pub fn assets_directory(&self) -> PathBuf {
        self.root.join("assets")
    }

    #[allow(dead_code)]
    pub fn cache_directory(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn logs_directory(&self) -> PathBuf {
        match self.mode {
            DataDirectoryMode::Portable => self.root.join("logs"),
            // Preserve the existing AppData log location for compatibility.
            DataDirectoryMode::AppData => self.root.clone(),
        }
    }

    pub fn steam_images_directory(&self) -> PathBuf {
        match self.mode {
            DataDirectoryMode::Portable => self.assets_directory().join("steam-images"),
            // Preserve the existing AppData image cache location for compatibility.
            DataDirectoryMode::AppData => self.root.join("steam-images"),
        }
    }

    pub fn ensure_root(&self) -> io::Result<()> {
        fs::create_dir_all(self.root())
    }

    pub(crate) fn storage_mode_marker_path(root: &Path) -> PathBuf {
        root.join(STORAGE_MODE_MARKER)
    }

    pub(crate) fn pending_migration_path(root: &Path) -> PathBuf {
        root.join(PENDING_MIGRATION_MARKER)
    }

    pub(crate) fn write_mode_marker(root: &Path, mode: DataDirectoryMode) -> io::Result<()> {
        fs::create_dir_all(root)?;
        let marker = Self::storage_mode_marker_path(root);
        let temporary = root.join(format!(".{STORAGE_MODE_MARKER}.tmp-{}", std::process::id()));
        fs::write(&temporary, mode.as_str())?;
        if marker.exists() {
            fs::remove_file(&marker)?;
        }
        fs::rename(temporary, marker)
    }

    pub(crate) fn write_pending_migration(
        root: &Path,
        migration: &PendingMigration,
    ) -> io::Result<()> {
        fs::create_dir_all(root)?;
        let marker = Self::pending_migration_path(root);
        let temporary = root.join(format!(
            ".{PENDING_MIGRATION_MARKER}.tmp-{}",
            std::process::id()
        ));
        let contents = serde_json::to_vec_pretty(migration)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        fs::write(&temporary, contents)?;
        if marker.exists() {
            fs::remove_file(&marker)?;
        }
        fs::rename(temporary, marker)
    }

    pub(crate) fn read_pending_migration(&self) -> Option<PendingMigration> {
        let contents = fs::read(Self::pending_migration_path(self.root())).ok()?;
        serde_json::from_slice(&contents).ok()
    }

    pub(crate) fn recovery_resolver(&self) -> Option<Self> {
        let pending = self.read_pending_migration()?;
        let source = PathBuf::from(pending.source_root);
        let source_mode = DataDirectoryMode::from_str(&pending.source_mode)?;
        if !self.is_known_root(&source) || source == self.root || !source.is_dir() {
            return None;
        }
        Some(Self::from_roots(
            source,
            source_mode,
            self.app_data_root.clone(),
            self.portable_root.clone(),
        ))
    }

    pub(crate) fn confirm_pending_migration(&self) -> io::Result<()> {
        let Some(pending) = self.read_pending_migration() else {
            return Ok(());
        };
        let source = PathBuf::from(&pending.source_root);
        if !self.is_known_root(&source) || source == self.root || !source.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "migration source is not a known LumaDeck storage root",
            ));
        }
        if pending.delete_source {
            fs::remove_dir_all(&source)?;
        }
        if let Some(backup) = pending.target_backup.as_deref() {
            let backup_path = PathBuf::from(backup);
            let parent = self.root.parent().unwrap_or_else(|| Path::new("."));
            let valid_backup = backup_path.starts_with(parent)
                && backup_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".lumadeck-migration-backup-"));
            if !valid_backup {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "migration backup is not a valid temporary path",
                ));
            }
            if backup_path.exists() {
                fs::remove_dir_all(backup_path)?;
            }
        }
        fs::remove_file(Self::pending_migration_path(self.root()))
    }

    fn from_roots(
        root: PathBuf,
        mode: DataDirectoryMode,
        app_data_root: PathBuf,
        portable_root: PathBuf,
    ) -> Self {
        Self {
            root,
            mode,
            app_data_root,
            portable_root,
        }
    }

    fn is_known_root(&self, root: &Path) -> bool {
        root == self.app_data_root
            || (!self.portable_root.as_os_str().is_empty() && root == self.portable_root)
    }
}

impl DataDirectoryMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AppData => "appData",
            Self::Portable => "portable",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "appData" | "appdata" | "normal" => Some(Self::AppData),
            "portable" => Some(Self::Portable),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataDirectoryMode, DataDirectoryResolver};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumadeck-data-directory-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn uses_app_data_when_portable_directory_is_absent() {
        let root = test_root("app-data");
        let executable = root.join("LumaDeck.exe");
        let app_data = root.join("app-data");
        let resolver = DataDirectoryResolver::new(&executable, &app_data);

        assert_eq!(resolver.mode(), DataDirectoryMode::AppData);
        assert_eq!(resolver.root(), app_data.as_path());
        assert_eq!(
            resolver.steam_images_directory(),
            app_data.join("steam-images")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn uses_sibling_data_directory_for_portable_mode() {
        let root = test_root("portable");
        let executable = root.join("LumaDeck.exe");
        let portable_root = root.join("data");
        let app_data = root.join("app-data");
        fs::create_dir_all(&portable_root).expect("portable directory should be created");

        let resolver = DataDirectoryResolver::new(&executable, &app_data);

        assert_eq!(resolver.mode(), DataDirectoryMode::Portable);
        assert_eq!(resolver.root(), portable_root.as_path());
        assert_eq!(resolver.database_path(), portable_root.join("lumadeck.db"));
        assert_eq!(resolver.assets_directory(), portable_root.join("assets"));
        assert_eq!(resolver.cache_directory(), portable_root.join("cache"));
        assert_eq!(resolver.logs_directory(), portable_root.join("logs"));
        assert_eq!(
            resolver.steam_images_directory(),
            portable_root.join("assets").join("steam-images")
        );

        let _ = fs::remove_dir_all(root);
    }
}
