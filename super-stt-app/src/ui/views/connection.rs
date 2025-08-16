// SPDX-License-Identifier: GPL-3.0-only
use super::common::page_layout;
use crate::state::DaemonStatus;
use crate::ui::messages::Message;
use cosmic::{
    Element,
    widget::{settings, text},
};

/// Settings page view using cosmic-settings style
pub fn page(
    daemon_status: &DaemonStatus,
    socket_path: String,
    udp_port: u16,
) -> Element<'_, Message> {
    let status_text = match daemon_status {
        DaemonStatus::Connected => "✅ Connected".to_string(),
        DaemonStatus::Connecting => "⏳ Connecting...".to_string(),
        DaemonStatus::Disconnected => "❌ Disconnected".to_string(),
        DaemonStatus::Error(err) => format!("❌ Error: {err}"),
    };

    let sections = vec![
        settings::section()
            .title("Connection Information")
            .add(settings::item("Connection", text::body(status_text)))
            .add(settings::item("Socket Path", text::body(socket_path)))
            .add(settings::item("UDP Port", text::body(udp_port.to_string())))
            .into(),
    ];

    let sections_view = settings::view_column(sections);
    page_layout("Connection", sections_view)
}
