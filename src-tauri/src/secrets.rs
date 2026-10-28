//! Account passwords live in the system's own password store, never in the mail database:
//! Windows Credential Manager, a Secret Service keyring on Linux (GNOME Keyring, KWallet), the
//! Keychain on macOS. Each one is stored under the service "Telex" and the account's email
//! address.

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error};

use telex_core::tr;

use crate::capitalize;

const SERVICE: &str = "Telex";

// The maintained keyring ecosystem separates platform drivers from the common API.
// Keep the existing service/user names, Windows UTF-16 secrets, and Secret Service
// attributes, so upgrading doesn't require entering saved credentials again.
fn native_entry(email: &str) -> Result<Entry, Error> {
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new()?;
    #[cfg(target_os = "linux")]
    let store = dbus_secret_service_keyring_store::Store::new()?;
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()?;
    store.build(SERVICE, email, None)
}

/// The store as messages name it, in the interface's language.
#[cfg(windows)]
fn store() -> String {
    tr!("Windows Credential Manager", "Menedżer poświadczeń Windows")
}
#[cfg(target_os = "macos")]
fn store() -> String {
    tr!("the macOS Keychain", "pęk kluczy macOS")
}
#[cfg(not(any(windows, target_os = "macos")))]
fn store() -> String {
    tr!("the system keyring", "systemowy magazyn haseł")
}

/// The store's error, and on Linux what to do about it: a Linux desktop may have no keyring
/// installed or running, or keep it locked, and the errors don't tell these apart.
fn explain(e: &Error) -> String {
    match e {
        Error::PlatformFailure(_) | Error::NoStorageAccess(_) if cfg!(target_os = "linux") => {
            let e = e.to_string();
            let e = e.trim_end_matches('.');
            tr!(
                "{e}. Check that a keyring such as GNOME Keyring or KWallet is installed, \
                 running and unlocked",
                "{e}. Sprawdź, czy magazyn haseł, np. GNOME Keyring albo KWallet, jest \
                 zainstalowany, uruchomiony i odblokowany"
            )
        }
        _ => e.to_string(),
    }
}

fn entry(email: &str) -> Result<Entry, String> {
    native_entry(email).map_err(|e| {
        let (store, why) = (store(), explain(&e));
        capitalize(&tr!(
            "{store} refused the entry: {why}",
            "{store} nie przyjął wpisu: {why}"
        ))
    })
}

pub fn save_password(email: &str, password: &str) -> Result<(), String> {
    entry(email)?.set_password(password).map_err(|e| {
        let (store, why) = (store(), explain(&e));
        tr!(
            "Couldn't save the password in {store}: {why}",
            "Nie udało się zapisać hasła ({store}): {why}"
        )
    })
}

pub fn load_password(email: &str) -> Result<String, String> {
    match entry(email)?.get_password() {
        Ok(p) => Ok(p),
        Err(Error::NoEntry) => Err(tr!(
            "no saved password. Add the account again to enter it.",
            "brak zapisanego hasła. Dodaj konto jeszcze raz, żeby je wpisać."
        )),
        Err(e) => {
            let (store, why) = (store(), explain(&e));
            Err(tr!(
                "couldn't read the password from {store}: {why}",
                "nie udało się odczytać hasła ({store}): {why}"
            ))
        }
    }
}

/// Forget the password. A missing entry is fine: there is nothing to forget.
pub fn delete_password(email: &str) {
    if let Ok(e) = entry(email) {
        let _ = e.delete_credential();
    }
}

/// The system store's entry for an OpenPGP key's passphrase (only when the user ticked
/// Remember): the same service, under "openpgp:<fingerprint>".
fn passphrase_entry(fingerprint: &str) -> Result<Entry, String> {
    entry(&format!("openpgp:{fingerprint}"))
}

pub fn save_passphrase(fingerprint: &str, passphrase: &str) -> Result<(), String> {
    passphrase_entry(fingerprint)?
        .set_password(passphrase)
        .map_err(|e| {
            let (store, why) = (store(), explain(&e));
            tr!(
                "Couldn't save the passphrase in {store}: {why}",
                "Nie udało się zapisać hasła klucza ({store}): {why}"
            )
        })
}

/// The remembered passphrase, if there is one (any failure is "none": Telex then asks).
pub fn load_passphrase(fingerprint: &str) -> Option<String> {
    passphrase_entry(fingerprint).ok()?.get_password().ok()
}

pub fn delete_passphrase(fingerprint: &str) {
    if let Ok(e) = passphrase_entry(fingerprint) {
        let _ = e.delete_credential();
    }
}

/// The system store's entry for an S/MIME certificate's password (only when the user ticked
/// Remember): the same service, under "smime:<fingerprint>".
fn cert_password_entry(fingerprint: &str) -> Result<Entry, String> {
    entry(&format!("smime:{fingerprint}"))
}

pub fn save_cert_password(fingerprint: &str, password: &str) -> Result<(), String> {
    cert_password_entry(fingerprint)?
        .set_password(password)
        .map_err(|e| {
            let (store, why) = (store(), explain(&e));
            tr!(
                "Couldn't save the certificate's password in {store}: {why}",
                "Nie udało się zapisać hasła certyfikatu ({store}): {why}"
            )
        })
}

/// The remembered password, if there is one (any failure is "none": Telex then asks).
pub fn load_cert_password(fingerprint: &str) -> Option<String> {
    cert_password_entry(fingerprint).ok()?.get_password().ok()
}

pub fn delete_cert_password(fingerprint: &str) {
    if let Ok(e) = cert_password_entry(fingerprint) {
        let _ = e.delete_credential();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredWriteW, CREDENTIALW, CRED_PERSIST_SESSION, CRED_TYPE_GENERIC,
    };

    // Write the old keyring 3.x representation directly through Windows, then read it
    // using the new driver. Only a unique synthetic test credential is ever accessed.
    #[test]
    fn reads_existing_windows_utf16_credential() {
        let service = format!(
            "Telex-migration-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let user = "synthetic@example.invalid";
        let mut target: Vec<u16> = format!("{user}.{service}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut username: Vec<u16> = user.encode_utf16().chain(Some(0)).collect();
        let expected = "żółć-🔑-日本語";
        let mut secret: Vec<u16> = expected.encode_utf16().collect();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            UserName: username.as_mut_ptr(),
            CredentialBlob: secret.as_mut_ptr().cast(),
            CredentialBlobSize: (secret.len() * 2) as u32,
            Persist: CRED_PERSIST_SESSION,
            ..Default::default()
        };
        // SAFETY: every pointer references a live, correctly sized UTF-16 buffer.
        assert_ne!(unsafe { CredWriteW(&credential, 0) }, 0);
        struct Cleanup(Vec<u16>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                // SAFETY: the target remains a live, nul-terminated UTF-16 string.
                unsafe {
                    CredDeleteW(self.0.as_ptr(), CRED_TYPE_GENERIC, 0);
                }
            }
        }
        let _cleanup = Cleanup(target);
        let entry = windows_native_keyring_store::Store::new()
            .unwrap()
            .build(&service, user, None)
            .unwrap();
        assert_eq!(entry.get_password().unwrap(), expected);
    }
}
