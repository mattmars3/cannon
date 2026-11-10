use std::{
    env,
    path::{Path, PathBuf},
};

pub const MAX_UPLOAD_BYTES: u64 = 2 << 32; // 4 GiB
pub const DELETE_ON_DOWNLOAD_QUERY_PARAM_NAME: &'static str = "deleteOnDownload";
pub const FILENAME_QUERY_PARAM_NAME: &'static str = "filename";
pub const UUID_QUERY_PARAM_NAME: &'static str = "uuid";
pub const DELETE_ON_DOWNLOAD_DEFAULT: bool = false;

pub const CANNON_DB_FILE_NAME: &str = "cannon.db";
pub const CANNON_FILES_DIR_NAME: &str = "data";

pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(d) = env::var_os("CANNON_DATA_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(d));
    }
    if let Some(dirs) = env::var_os("STATE_DIRECTORY").filter(|v| !v.is_empty()) {
        // systemd allows several dirs, colon separated; the first is ours.
        if let Some(first) = dirs.to_string_lossy().split(':').find(|p| !p.is_empty()) {
            return Ok(PathBuf::from(first));
        }
    }
    xdg::BaseDirectories::with_prefix("cannon")
        .get_state_home()
        .ok_or_else(|| "cannot determine data directory; set CANNON_DATA_DIR".to_string())
}

// references the cannon.db canonical path
pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CANNON_DB_FILE_NAME)
}

// references the data/ directory where files are stored
pub fn files_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CANNON_FILES_DIR_NAME)
}
