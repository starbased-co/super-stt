// SPDX-License-Identifier: GPL-3.0-only
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// UDP Authentication using a shared secret file
///
/// This provides authentication for UDP connections where process credentials
/// are not available. A shared secret is generated and stored in a file
/// accessible only by the user.
#[derive(Clone)]
pub struct UdpAuth {
    secret_file: PathBuf,
}

impl UdpAuth {
    /// Create a new UDP authenticator
    ///
    /// # Errors
    /// This function will return an error if the secret file cannot be created.
    pub fn new() -> Result<Self> {
        let secret_file = Self::get_secret_file_path()?;
        let auth = Self { secret_file };

        // CRITICAL: Generate/load secret immediately to avoid race conditions
        // This ensures the secret file exists before any clients try to read it
        let _secret = auth.get_or_create_secret()?;
        log::debug!("UDP authentication initialized with secret at {:?}", auth.secret_file);

        Ok(auth)
    }

    /// Get the path to the secret file
    fn get_secret_file_path() -> Result<PathBuf> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .or_else(|_| std::env::var("TMPDIR"))
            .unwrap_or_else(|_| "/tmp".to_string());

        let secret_dir = PathBuf::from(runtime_dir).join("stt");

        // Create directory if it doesn't exist
        if !secret_dir.exists() {
            fs::create_dir_all(&secret_dir).context("Failed to create secret directory")?;

            // Set restrictive permissions (owner only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::Permissions::from_mode(0o700);
                fs::set_permissions(&secret_dir, perms)
                    .context("Failed to set directory permissions")?;
            }
        }

        Ok(secret_dir.join("udp_secret"))
    }

    /// Generate or load the shared secret
    ///
    /// # Errors
    /// This function will return an error if the secret file cannot be read.
    pub fn get_or_create_secret(&self) -> Result<String> {
        if self.secret_file.exists() {
            // Load existing secret
            self.load_secret()
        } else {
            // Generate new secret
            self.generate_secret()
        }
    }

    fn load_secret(&self) -> Result<String> {
        let secret = fs::read_to_string(&self.secret_file).context("Failed to read secret file")?;
        Ok(secret.trim().to_string())
    }

    /// Generate a new random secret and save it
    ///
    /// # Errors
    /// This function will return an error if the secret file cannot be read.
    fn generate_secret(&self) -> Result<String> {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Generate a simple but unpredictable secret using timestamp and process ID
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        let secret = format!("stt_{timestamp}_{pid}");

        // Write to file with restrictive permissions
        fs::write(&self.secret_file, &secret).context("Failed to write secret file")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.secret_file, perms)
                .context("Failed to set secret file permissions")?;
        }

        log::info!("Generated new UDP authentication secret");
        Ok(secret)
    }

    /// Create an authenticated registration message
    ///
    /// # Errors
    /// This function will return an error if the secret file cannot be read.
    pub fn create_auth_message(&self, client_type: &str) -> Result<String> {
        let secret = self.get_or_create_secret()?;
        Ok(format!("REGISTER:{client_type}:{secret}"))
    }

    /// Verify an authenticated registration message
    ///
    /// # Errors
    /// This function will return an error if the secret file cannot be read.
    pub fn verify_auth_message(&self, message: &str) -> Result<Option<String>> {
        let secret = self.get_or_create_secret()?;

        if let Some(rest) = message.strip_prefix("REGISTER:")
            && let Some((client_type, provided_secret)) = rest.split_once(':')
        {
            if provided_secret == secret {
                return Ok(Some(client_type.to_string()));
            } else {
                log::warn!("UDP authentication failed: secret mismatch");
            }
        }

        Ok(None)
    }

    /// Clean up the secret file (e.g., on daemon shutdown)
    ///
    /// # Errors
    /// This function will return an error if the secret file cannot be removed.
    pub fn cleanup(&self) -> Result<()> {
        if self.secret_file.exists() {
            fs::remove_file(&self.secret_file).context("Failed to remove secret file")?;
            log::info!("Cleaned up UDP authentication secret");
        }
        Ok(())
    }
}

impl Default for UdpAuth {
    fn default() -> Self {
        Self::new().expect("Failed to initialize UDP authentication")
    }
}
