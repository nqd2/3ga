pub mod ply;
pub mod splat;
pub mod sog;

use crate::error::{AgError, AgResult};
use crate::splat_table::SplatTable;
use std::path::Path;

pub use ply::{read_ply, read_ply_bytes};
pub use splat::{read_splat, read_splat_bytes, write_splat_bytes};
pub use sog::{read_sog_bundle, read_sog_meta, write_sog_bundle};

pub fn read_source(path: impl AsRef<Path>) -> AgResult<(SplatTable, String)> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "splat" => read_splat(path).map(|table| (table, "splat".to_string())),
        "ply" => read_ply(path).map(|table| (table, "ply".to_string())),
        "sog" => read_sog_bundle(path).map(|table| (table, "sog".to_string())),
        "json" if path.file_name().and_then(|s| s.to_str()) == Some("meta.json") => {
            read_sog_meta(path).map(|table| (table, "sog".to_string()))
        }
        _ => Err(AgError::UnsupportedFormat(ext)),
    }
}
