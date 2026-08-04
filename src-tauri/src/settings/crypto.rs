use thiserror::Error;

#[derive(Debug, Error)]
#[cfg_attr(windows, allow(dead_code))]
pub enum CredentialError {
    #[error("credential protection is unavailable on this platform")]
    UnsupportedPlatform,
    #[error("credential protection failed")]
    ProtectionFailed,
    #[error("credential could not be unprotected")]
    UnprotectionFailed,
}

#[cfg(windows)]
pub fn protect(value: &str) -> Result<Vec<u8>, CredentialError> {
    use std::{mem::size_of, ptr::null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

    let bytes = value.as_bytes();
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let result = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut output,
        )
    };
    if result == 0 || output.pbData.is_null() || output.cbData == 0 {
        return Err(CredentialError::ProtectionFailed);
    }
    let encrypted =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData as *mut core::ffi::c_void);
    }
    let _ = size_of::<CRYPT_INTEGER_BLOB>();
    Ok(encrypted)
}

#[cfg(not(windows))]
pub fn protect(_value: &str) -> Result<Vec<u8>, CredentialError> {
    Err(CredentialError::UnsupportedPlatform)
}

#[cfg(windows)]
pub fn unprotect(value: &[u8]) -> Result<String, CredentialError> {
    use std::{ptr::null_mut, slice};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let input = CRYPT_INTEGER_BLOB {
        cbData: value.len() as u32,
        pbData: value.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let result = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            &mut output,
        )
    };
    if result == 0 || output.pbData.is_null() {
        return Err(CredentialError::UnprotectionFailed);
    }
    let plaintext =
        unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData as *mut core::ffi::c_void);
    }
    String::from_utf8(plaintext).map_err(|_| CredentialError::UnprotectionFailed)
}

#[cfg(not(windows))]
pub fn unprotect(_value: &[u8]) -> Result<String, CredentialError> {
    Err(CredentialError::UnsupportedPlatform)
}

#[cfg(all(test, windows))]
mod tests {
    use super::{protect, unprotect};

    #[test]
    fn dpapi_round_trip_does_not_store_plaintext() {
        let value = "ABCDEFGHIJKLMNOP";
        let encrypted = protect(value).expect("DPAPI protection");
        assert!(!encrypted.is_empty());
        assert!(!String::from_utf8_lossy(&encrypted).contains(value));
        assert_eq!(unprotect(&encrypted).expect("DPAPI unprotection"), value);
    }
}
