// SPDX-License-Identifier: GPL-3.0-only
#![allow(unused)]

use tokio::io::{self, AsyncWriteExt, stdout};

// ANSI color codes
pub const BLACK: &str = "\x1b[0;30m";
pub const RED: &str = "\x1b[0;31m";
pub const GREEN: &str = "\x1b[0;32m";
pub const YELLOW: &str = "\x1b[0;33m";
pub const BLUE: &str = "\x1b[0;34m";
pub const MAGENTA: &str = "\x1b[0;35m";
pub const CYAN: &str = "\x1b[0;36m";
pub const WHITE: &str = "\x1b[0;37m";
pub const NC: &str = "\x1b[0m"; // No Color

// ANSI bold colors
pub const BOLD_BLACK: &str = "\x1b[1;30m";
pub const BOLD_RED: &str = "\x1b[1;31m";
pub const BOLD_GREEN: &str = "\x1b[1;32m";
pub const BOLD_YELLOW: &str = "\x1b[1;33m";
pub const BOLD_BLUE: &str = "\x1b[1;34m";
pub const BOLD_MAGENTA: &str = "\x1b[1;35m";
pub const BOLD_CYAN: &str = "\x1b[1;36m";
pub const BOLD_WHITE: &str = "\x1b[1;37m";
pub const BOLD_NC: &str = "\x1b[1;37m";

// ANSI background color codes
pub const BG_BLACK: &str = "\x1b[1;40;37m";
pub const BG_RED: &str = "\x1b[1;41;37m";
pub const BG_GREEN: &str = "\x1b[1;42;30m";
pub const BG_YELLOW: &str = "\x1b[1;43;30m";
pub const BG_BLUE: &str = "\x1b[1;44;37m";
pub const BG_MAGENTA: &str = "\x1b[1;45;37m";
pub const BG_CYAN: &str = "\x1b[1;46;37m";
pub const BG_WHITE: &str = "\x1b[1;47;30m";
pub const BG_NC: &str = "\x1b[1;49;37m"; // No Color

// Logging constants
const LOG_INFO: &str = "\x1b[1;32mINFO    \x1b[0m";
const LOG_ERROR: &str = "\x1b[1;31mERROR   \x1b[0m";
const LOG_WARN: &str = "\x1b[1;33mWARN    \x1b[0m";
const LOG_DEBUG: &str = "\x1b[1;34mDEBUG   \x1b[0m";

// Function to print messages in color
pub async fn print_color(color: &str, message: &str) {
    // println!("{}{}{}", color, message, NC);
    let parsed_message = format!("{color}{message}{NC}\n");
    let mut stdout = stdout();
    if let Err(e) = stdout.write_all(parsed_message.as_bytes()).await {
        println!("Failed to write to stdout. Defaulting to println!(). Error: {e}");
        println!("{parsed_message}");
    }
    if let Err(e) = stdout.flush().await {
        println!("Failed to flush stdout. Error: {e}");
    }
}

// Logging functions
pub async fn debug(message: &str) {
    print_color(LOG_DEBUG, message).await;
}

pub async fn info(message: &str) {
    print_color(LOG_INFO, message).await;
}

pub async fn warn(message: &str) {
    print_color(LOG_WARN, message).await;
}

pub async fn error(message: &str) {
    print_color(LOG_ERROR, message).await;
}

/// Print a newline
pub async fn nl() {
    let mut stdout = stdout();
    if let Err(e) = stdout.write_all("\n".as_bytes()).await {
        println!("Failed to write to stdout. Defaulting to println!(). Error: {e}");
        println!("\n");
    }
    if let Err(e) = stdout.flush().await {
        println!("Failed to flush stdout. Error: {e}");
    }
}
