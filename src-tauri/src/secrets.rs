//! Account passwords live in Windows Credential Manager, never in the mail database.
//! Each one is stored under the service "Telex" and the account's email address.

use keyring::Entry;

const SERVICE: &str = "Telex";

fn entry(email: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, email)
        .map_err(|e| format!("Windows Credential Manager refused the entry: {e}"))
}

pub fn save_password(email: &str, password: &str) -> Result<(), String> {
    entry(email)?
        .set_password(password)
        .map_err(|e| format!("Couldn't save the password in Windows Credential Manager: {e}"))
}

pub fn load_password(email: &str) -> Result<String, String> {
    match entry(email)?.get_password() {
        Ok(p) => Ok(p),
        Err(keyring::Error::NoEntry) => {
            Err("no saved password. Add the account again to enter it.".into())
        }
        Err(e) => Err(format!(
            "couldn't read the password from Windows Credential Manager: {e}"
        )),
    }
}

/// Forget the password. A missing entry is fine: there is nothing to forget.
pub fn delete_password(email: &str) {
    if let Ok(e) = entry(email) {
        let _ = e.delete_credential();
    }
}
