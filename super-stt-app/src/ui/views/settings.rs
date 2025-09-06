// SPDX-License-Identifier: GPL-3.0-only
use cosmic::Element;
use cosmic::iced::Length;
use cosmic::iced_widget::{column, row};
use cosmic::widget::{self, button, settings, text};
use super_stt_shared::theme::AudioTheme;
// Reuse shared models
use super_stt_shared::{models::protocol::DownloadProgress, stt_model::STTModel};

use super::common::page_layout;
use crate::ui::messages::Message;

/// Preview typing settings section using cosmic-settings style
pub fn preview_typing_settings_widget(preview_typing_enabled: bool) -> Element<'static, Message> {
    let mut section = settings::section().title("Preview Typing (Beta)");

    // Add description text as a separate item
    section = section.add(settings::item(
        "",
        text::caption("Preview typing shows transcription results as you speak. This is an experimental feature and may affect performance.")
    ));

    // Add the toggler control
    section = section.add(settings::item(
        "Enable Preview Typing",
        cosmic::widget::toggler(preview_typing_enabled).on_toggle(Message::PreviewTypingToggled),
    ));

    section.into()
}

/// Audio themes page view using cosmic-settings style
pub fn audio_theme_selection_widget<'a>(
    audio_themes: &'a [AudioTheme],
    selected_audio_theme: &'a AudioTheme,
) -> Element<'a, Message> {
    // Create the theme names vector for the dropdown
    let theme_names: Vec<String> = audio_themes.iter().map(AudioTheme::pretty_name).collect();

    // Find the selected index
    let selected_index = audio_themes
        .iter()
        .position(|theme| theme == selected_audio_theme);

    // Create dropdown with proper message mapping
    let audio_themes_clone = audio_themes.to_vec();

    let theme_dropdown: Element<'a, Message> = if audio_themes.is_empty() {
        text::caption("Loading themes...").into()
    } else {
        widget::dropdown(theme_names, selected_index, move |index| {
            if let Some(&theme) = audio_themes_clone.get(index) {
                Message::AudioThemeSelected(theme)
            } else {
                Message::AudioThemeSelected(AudioTheme::Classic)
            }
        })
        .into()
    };

    settings::section()
        .title("Audio Theme")
        .add(settings::item("Theme", theme_dropdown))
        .into()
}

/// Create the download progress widget using cosmic-settings style
#[allow(clippy::cast_precision_loss)]
fn download_progress_widget(
    download_progress: Option<&DownloadProgress>,
    download_active: bool,
) -> Option<Element<'_, Message>> {
    if let Some(progress) = download_progress {
        // Security: Validate download progress values to prevent UI corruption
        // Allow percentage > 0 even when total_bytes is 0 (file size might not be known initially)

        // Only reject clearly invalid percentages
        if progress.percentage < 0.0 || progress.percentage > 100.0 {
            log::warn!(
                "Invalid progress percentage: {} (must be 0-100)",
                progress.percentage
            );
            return None;
        }

        // Allow file_index == total_files for the last file, only reject if clearly out of bounds
        if progress.total_files > 0 && progress.file_index > progress.total_files {
            log::warn!(
                "Invalid file index: {} > total_files {}",
                progress.file_index,
                progress.total_files
            );
            return None;
        }

        // Safe calculation with bounds checking
        let progress_fraction = (progress.percentage / 100.0).clamp(0.0, 1.0);

        let progress_text = if progress.status == "downloading" {
            format!(
                "Downloading {} ({}/{}): {:.1}%",
                progress.model_name,
                progress.file_index + 1,
                progress.total_files,
                progress.percentage
            )
        } else {
            format!("{}: {}", progress.model_name, progress.status)
        };

        let eta_text = if let Some(eta_seconds) = progress.eta_seconds {
            if eta_seconds > 0 {
                let minutes = eta_seconds / 60;
                let seconds = eta_seconds % 60;
                if minutes > 0 {
                    format!("ETA: {minutes}m {seconds}s")
                } else {
                    format!("ETA: {seconds}s")
                }
            } else {
                "Finishing...".to_string()
            }
        } else {
            String::new()
        };

        let bytes_text = if progress.total_bytes > 0 {
            let mb_downloaded = progress.bytes_downloaded as f64 / (1024.0 * 1024.0);
            let mb_total = progress.total_bytes as f64 / (1024.0 * 1024.0);
            format!("{mb_downloaded:.1} / {mb_total:.1} MB")
        } else {
            String::new()
        };

        let details_widget = column![
            text::body(progress_text),
            widget::progress_bar(
                0.0..=1.0,
                // Audio level can be a minimum of 0.1 when recording because lower than that and it can overflow when theme is fully rounded.
                progress_fraction.max(if progress.total_bytes > 0 { 0.1 } else { 0.0 }),
            )
            .width(Length::Fill),
            row![
                text::body(bytes_text).width(Length::Fill),
                text::body(eta_text).width(Length::Fill),
            ]
            .spacing(10),
        ]
        .spacing(10);

        let mut section = settings::section()
            .title("Speech-to-Text Model")
            .add(settings::flex_item("Status", details_widget));

        // Add cancel button only if download is active
        if download_active && progress.status == "downloading" {
            section = section.add(settings::item(
                "Cancel",
                button::destructive("Cancel Download").on_press(Message::CancelDownload),
            ));
        }

        Some(section.into())
    } else {
        None
    }
}

/// Create the model selection widget using cosmic-settings style
fn model_selection_settings_widget<'a>(
    available_models: &'a [STTModel],
    current_model: &'a STTModel,
    download_active: bool,
    current_device: &'a str,
    available_devices: &'a [String],
    device_switching: bool,
) -> Element<'a, Message> {
    let mut section = settings::section().title("Speech-to-Text Model");

    // Model selection dropdown
    if available_models.is_empty() {
        section = section.add(settings::item("Model", text::caption("Loading models...")));
    } else {
        let selected_index = available_models
            .iter()
            .position(|model| model == current_model);

        let model_names: Vec<String> = available_models
            .iter()
            .map(std::string::ToString::to_string)
            .collect();

        if download_active {
            section = section.add(settings::item(
                "Model",
                text::caption("Model switching disabled during download"),
            ));
        } else {
            let available_models_clone = available_models.to_vec();
            section = section.add(settings::item(
                "Model",
                widget::dropdown(model_names, selected_index, move |index| {
                    if let Some(model) = available_models_clone.get(index) {
                        Message::ModelSelected(*model)
                    } else {
                        Message::ModelError("Invalid model selection".to_string())
                    }
                }),
            ));

            // Map devices to user-friendly options
            let device_options: Vec<(&str, &str)> = available_devices
                .iter()
                .map(|device| {
                    if device == "CPU" {
                        ("cpu", "CPU (slower, always available)")
                    } else {
                        ("cuda", "CUDA GPU (faster if available)")
                    }
                })
                .collect();

            let device_names: Vec<String> = device_options
                .iter()
                .map(|(_, name)| (*name).to_string())
                .collect();

            let selected_device_index = device_options
                .iter()
                .position(|(device_id, _)| device_id == &current_device);

            let device_selection_widget: Element<'a, Message> =
                if device_switching || download_active {
                    text::caption("Device switching disabled during operation").into()
                } else {
                    let device_options_clone = device_options.clone();
                    widget::dropdown(device_names, selected_device_index, move |index| {
                        if let Some((device_id, _)) = device_options_clone.get(index) {
                            Message::DeviceSelected((*device_id).to_string())
                        } else {
                            Message::DeviceError("Invalid device selection".to_string())
                        }
                    })
                    .into()
                };

            section = section.add(settings::item("Device", device_selection_widget));

            // Add warning message based on available devices and current state
            let warning_message = if available_devices.len() == 1 && available_devices[0] == "CPU" {
                "Note: This build does not include GPU support"
            } else if current_device == "cpu" && available_devices.contains(&"GPU".to_string()) {
                "Note: GPU acceleration may be unavailable - check CUDA installation"
            } else {
                "Note: GPU will fallback to CPU if unavailable or insufficient memory"
            };

            section = section.add(settings::item("", text::caption(warning_message)));
        }
    }

    section.into()
}

/// Settings page view using cosmic-settings style
#[allow(clippy::too_many_arguments)]
pub fn page<'a>(
    audio_themes: &'a [AudioTheme],
    selected_audio_theme: &'a AudioTheme,
    available_models: &'a [STTModel],
    current_model: &'a STTModel,
    download_progress: Option<&'a DownloadProgress>,
    download_active: bool,
    current_device: &'a str,
    available_devices: &'a [String],
    device_switching: bool,
    preview_typing_enabled: bool,
) -> Element<'a, Message> {
    let mut sections = Vec::new();

    sections.push(audio_theme_selection_widget(
        audio_themes,
        selected_audio_theme,
    ));

    // Add preview typing section
    sections.push(preview_typing_settings_widget(preview_typing_enabled));

    // Download Progress Section (only if active)
    if let Some(progress_widget) = download_progress_widget(download_progress, download_active) {
        sections.push(progress_widget);
    } else {
        // Model Selection Section
        sections.push(model_selection_settings_widget(
            available_models,
            current_model,
            download_active,
            current_device,
            available_devices,
            device_switching,
        ));
    }
    let sections_view = settings::view_column(sections);
    page_layout("Settings", sections_view)
}
