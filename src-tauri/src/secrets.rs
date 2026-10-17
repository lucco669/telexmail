//! Account passwords live in the system's own password store, never in the mail database:
//! Windows Credential Manager, a Secret Service keyring on Linux (GNOME Keyring, KWallet), the
//! Keychain on macOS. Each one is stored under the service "Telex" and the account's email
//! address.

use keyring::{Entry, Error};

use telex_core::tr;

use crate::capitalize;

const SERVICE: &str = "Telex";

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
    Entry::new(SERVICE, email).map_err(|e| {
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
