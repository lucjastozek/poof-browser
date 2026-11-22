//! Crash-safe JSON file writes shared by state.json and history.json.

use serde::Serialize;
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
};

/// Streams `value` as compact JSON into a temp file, fsyncs it, optionally keeps the previous
/// file as `.bak`, then swaps the new one in. Returns whether the write succeeded.
/// Serializing straight into a buffered file avoids holding the whole document in memory.
pub(super) fn write_json_atomically<T: Serialize + ?Sized>(
    value: &T,
    path: &Path,
    keep_backup: bool,
) -> bool {
    let tmp = path.with_extension("json.tmp");
    let written = (|| -> std::io::Result<()> {
        let mut w = BufWriter::new(fs::File::create(&tmp)?);
        serde_json::to_writer(&mut w, value)?;
        w.flush()?;
        w.get_ref().sync_all()?;
        Ok(())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if keep_backup && path.exists() {
        let _ = fs::copy(path, path.with_extension("json.bak"));
    }
    fs::rename(&tmp, path).is_ok()
}
