pub const MAX_UPLOAD_BYTES: u64 = 2 << 32; // 4 GiB
pub const DELETE_ON_DOWNLOAD_QUERY_PARAM_NAME: &'static str = "deleteOnDownload";
pub const FILENAME_QUERY_PARAM_NAME: &'static str = "filename";
pub const UUID_QUERY_PARAM_NAME: &'static str = "uuid";
pub const DELETE_ON_DOWNLOAD_DEFAULT: bool = false;
// TODO: Add XDG support so it picks a smart place on mulitple platforms.
pub const CANNON_DB_PATH: &'static str = "/home/matt/.local/state/cannon/cannon_db.sqlite";
pub const CANNON_DATA_PATH: &'static str = "/home/matt/.local/state/cannon/data";
