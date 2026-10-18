//! Real-Debrid API key in the OS credential store (Windows Credential
//! Manager / macOS Keychain). The key never touches settings files or logs.
use anyhow::{Result, anyhow};
use keyring::{Entry, Error};

const SERVICE: &str = "NSWpedia Search";
const ACCOUNT: &str = "real-debrid-api-key";

pub const STORE_NAME: &str = if cfg!(windows) {
    "Windows Credential Manager"
} else if cfg!(target_os = "macos") {
    "macOS Keychain"
} else {
    "memory only — not saved"
};

fn entry() -> Result<Entry> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| anyhow!("Credential store unavailable: {e}"))
}

pub fn load() -> Result<Option<String>> {
    match entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!("Could not read the saved API key: {e}")),
    }
}

pub fn save(token: &str) -> Result<()> {
    entry()?
        .set_password(token)
        .map_err(|e| anyhow!("Could not save the API key: {e}"))
}

pub fn delete() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!("Could not remove the API key: {e}")),
    }
}
