use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crate::util::file_error::{FileError, ResultWithFile};

pub static MAGIC_BYTES_VERSION_1: [u8; 8] = *b"\0asm\x01\0\0\0";

/// Quick check if a file could be a Wasm binary by its magic bytes and version.
pub fn is_wasm_by_magic_bytes(file: impl AsRef<Path>) -> Result<bool, FileError<io::Error>> {
    // Read the first 8 bytes of the (supposed) Wasm binary.
    let mut f = File::open(&file).with_file(&file)?;
    let mut buf = [0u8; 8];
    let result = f.read_exact(&mut buf);

    // Cannot be a valid wasm file because it has less than 8 bytes.
    if let Err(e) = &result {
        if let io::ErrorKind::UnexpectedEof = e.kind() {
            return Ok(false);
        }
    }

    // Propagate other IO errors.
    result.with_file(&file)?;

    // Check magic bytes and version.
    Ok(buf == MAGIC_BYTES_VERSION_1)
}
