use crate::error::AgResult;
use crate::manifest::Manifest;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub fn write_webar_zip(
    out_path: impl AsRef<Path>,
    manifest: &Manifest,
    files: &[(&str, PathBuf)],
) -> AgResult<()> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let manifest_options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("manifest.json", manifest_options)?;
        zip.write_all(&serde_json::to_vec_pretty(manifest)?)?;
        for (name, path) in files {
            zip.start_file(*name, options)?;
            zip.write_all(&fs::read(path)?)?;
        }
        zip.finish()?;
    }
    fs::write(out_path, cursor.into_inner())?;
    Ok(())
}

pub fn playcanvas_runtime() -> &'static [u8] {
    include_bytes!("./webar/playcanvas.min.js")
}

pub fn playcanvas_license() -> &'static [u8] {
    include_bytes!("./webar/playcanvas.LICENSE.txt")
}

pub fn viewer_html() -> &'static str {
    include_str!("./webar/viewer.html")
}
