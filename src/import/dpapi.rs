//! Windows data protection (DPAPI): Chromium-based browsers, Arc included, protect their
//! password key with the current Windows user's credentials.

use anyhow::Result;

#[cfg(windows)]
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use windows::Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB},
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len())?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` points at `data`, which outlives the call. On success `output` is a
    // buffer allocated by Windows: copied out, then freed exactly once with `LocalFree`.
    unsafe {
        CryptUnprotectData(&input, None, None, None, None, 0, &mut output)?;
        let copy = if output.pbData.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(copy)
    }
}

/// Other systems protect the key differently (the macOS Keychain); not supported yet.
#[cfg(not(windows))]
pub fn unprotect(_data: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("reading Arc's saved passwords is only supported on Windows for now")
}
