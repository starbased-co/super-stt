// SPDX-License-Identifier: GPL-3.0-only
use std::path::PathBuf;
use std::sync::OnceLock;
use super_stt_shared::theme::AudioTheme;

// Generate a unique client ID for this applet instance
static CLIENT_ID: OnceLock<String> = OnceLock::new();

fn get_client_id() -> &'static str {
    CLIENT_ID
        .get_or_init(|| super_stt_shared::validation::generate_secure_client_id("super-stt-applet"))
}

/// Ping daemon to check if it's running and responsive
pub async fn ping_daemon(socket_path: PathBuf) -> Result<String, String> {
    super_stt_shared::daemon::client::ping_daemon(socket_path, get_client_id()).await
}

/// Set and test audio theme - convenience function
pub async fn set_and_test_audio_theme(
    socket_path: PathBuf,
    theme: String,
) -> Result<String, String> {
    super_stt_shared::daemon::client::set_and_test_audio_theme(socket_path, &theme, get_client_id())
        .await
}

/// Get current daemon configuration
pub async fn fetch_daemon_config(socket_path: PathBuf) -> Result<serde_json::Value, String> {
    super_stt_shared::daemon::client::fetch_daemon_config(socket_path, get_client_id()).await
}

/// Ping daemon and get extended connection status information
pub async fn ping_daemon_with_status(
    socket_path: PathBuf,
) -> Result<super_stt_shared::daemon::client::PingResponse, String> {
    super_stt_shared::daemon::client::ping_daemon_with_status(socket_path, get_client_id()).await
}

/// Load available audio themes from daemon with fallback
pub async fn load_audio_themes(socket_path: PathBuf) -> Vec<AudioTheme> {
    // Try to get available themes from daemon
    if let Ok(themes) = list_available_audio_themes(socket_path.clone()).await {
        return themes;
    }

    // Fallback to all available themes if daemon is unavailable
    AudioTheme::all_themes()
}

/// List available audio themes from daemon
pub async fn list_available_audio_themes(socket_path: PathBuf) -> Result<Vec<AudioTheme>, String> {
    let theme_strings =
        super_stt_shared::daemon::client::list_available_audio_themes(socket_path, get_client_id())
            .await?;

    // Convert strings back to AudioTheme enum
    let themes = theme_strings
        .into_iter()
        .filter_map(|theme_str| theme_str.parse::<AudioTheme>().ok())
        .collect();

    Ok(themes)
}
