//! `thundercrab` — desktop GUI for the ThunderCrab mail client.
//!
//! First-window scaffold. The app boots into a connection screen
//! that takes the IMAP host / user / password, calls
//! `RustImapBackend::connect`, and displays the folder list.
//!
//! ## Why Iced
//!
//! Toolkit chosen 2026-04-27 from four candidates (GTK4, Iced,
//! Tauri, Slint). The decision criteria were the supersociety
//! standard — pure Rust, single binary per platform, compile-time-
//! typed state, no embedded WebView or system GUI runtime.
//!
//! Iced wins because:
//!
//!   * Pure-Rust single binary on Linux / macOS / Windows. No GTK
//!     system runtime, no WebView2 / WebKitGTK pull-in, no JS
//!     engine surface in a privacy-pitched mail client.
//!   * Elm architecture — Message + State + update + view. Every
//!     transition is a typed enum case; the compiler refuses
//!     ambiguous state shapes. Same discipline as our typed Sieve
//!     rules and our Backend trait.
//!   * Cross-platform without bundling a 200MB system runtime.
//!     Distribution is one binary per platform; the Loom GTK
//!     theme generator stays useful for any GTK app the user
//!     runs alongside ThunderCrab.
//!   * accesskit is the active a11y story; not GTK-grade today,
//!     improving fast, sufficient for v0.
//!
//! ## What this does today
//!
//! Boots into a connection form. Submitting starts a
//! `RustImapBackend::connect` + `list_folders` call; the folder
//! list renders below the form. No mail-reading, no rules editing,
//! no Sieve push yet — those land as new screens once the wire-
//! to-UI loop is verified.
//!
//! Command to run:
//!
//! ```bash
//! cargo run -p thundercrab-gui
//! ```

#![doc(html_no_source)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

use iced::widget::{
    Column, button, column, container, row, scrollable, text, text_input,
};
use iced::{Element, Length, Task, Theme};
use thundercrab_imap::{
    AccountConfig, Backend, FolderSummary, rust_imap::RustImapBackend,
};

/// Entry point.
///
/// # Errors
/// Returns Iced's startup error if the runtime cannot be initialized.
pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .compact()
        .init();

    iced::application(App::default, App::update, App::view)
        .title("ThunderCrab")
        .theme(theme)
        .run()
}

/// Fixed light theme. A named fn (not a closure) so the `&App` lifetime is
/// universally quantified — a closure here infers one specific lifetime and
/// trips iced's `for<'a>` theme bound ("implementation of `Fn` is not general
/// enough").
fn theme(_state: &App) -> Theme {
    Theme::Light
}

/// App state. Single struct; transitions are pure functions of
/// (state, message) → (state, command).
#[derive(Default)]
struct App {
    /// Connection form host.
    host: String,
    /// Connection form user.
    user: String,
    /// Connection form password (cleared after a successful
    /// connect; never persisted).
    password: String,
    /// Loaded folders, populated after a successful connect.
    folders: Vec<FolderSummary>,
    /// Status / error string for the last connection attempt.
    status: String,
    /// True while a connect+list operation is in flight.
    connecting: bool,
}

#[derive(Debug, Clone)]
enum Message {
    HostChanged(String),
    UserChanged(String),
    PasswordChanged(String),
    Connect,
    Connected(Result<Vec<FolderSummary>, String>),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::HostChanged(s) => {
                self.host = s;
                Task::none()
            }
            Message::UserChanged(s) => {
                self.user = s;
                Task::none()
            }
            Message::PasswordChanged(s) => {
                self.password = s;
                Task::none()
            }
            Message::Connect => {
                if self.connecting {
                    return Task::none();
                }
                self.connecting = true;
                self.status = "Connecting…".into();
                let cfg = AccountConfig::plausiden(self.host.clone(), self.user.clone());
                let pw = std::mem::take(&mut self.password);
                Task::perform(connect_and_list(cfg, pw), Message::Connected)
            }
            Message::Connected(Ok(folders)) => {
                self.connecting = false;
                self.folders = folders;
                self.status = format!("Connected. {} folders.", self.folders.len());
                Task::none()
            }
            Message::Connected(Err(e)) => {
                self.connecting = false;
                self.folders.clear();
                self.status = format!("Failed: {e}");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let header = column![
            text("ThunderCrab").size(32),
            text("Local-first mail client. Rules transparent. No cloud.").size(14),
        ]
        .spacing(4);

        let form = column![
            row![
                text("IMAP host").width(Length::Fixed(120.0)),
                text_input("mail.example.com", &self.host)
                    .on_input(Message::HostChanged)
                    .padding(8),
            ]
            .spacing(8),
            row![
                text("Username").width(Length::Fixed(120.0)),
                text_input("you@example.com", &self.user)
                    .on_input(Message::UserChanged)
                    .padding(8),
            ]
            .spacing(8),
            row![
                text("Password").width(Length::Fixed(120.0)),
                text_input("…", &self.password)
                    .secure(true)
                    .on_input(Message::PasswordChanged)
                    .padding(8),
            ]
            .spacing(8),
            row![
                button(text(if self.connecting { "Connecting…" } else { "Connect" }))
                    .on_press_maybe(if self.connecting {
                        None
                    } else {
                        Some(Message::Connect)
                    }),
                text(&self.status).size(13),
            ]
            .spacing(12),
        ]
        .spacing(8);

        let folder_list = if self.folders.is_empty() {
            Column::new().push(text("(no folders loaded yet)"))
        } else {
            let mut col = Column::new().push(text("Folders").size(20));
            for f in &self.folders {
                col = col.push(
                    row![
                        text(format!("{:>4}", f.unseen)).width(Length::Fixed(48.0)),
                        text("/").width(Length::Fixed(16.0)),
                        text(format!("{:>5}", f.messages)).width(Length::Fixed(60.0)),
                        text(&f.name),
                    ]
                    .spacing(8),
                );
            }
            col
        };

        let body = column![header, form, scrollable(folder_list)]
            .spacing(24)
            .padding(24);

        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Async helper: connect, list folders, return as Result. The error
/// type is String because BackendError doesn't impl Clone (the
/// transport variants own non-clonable inner errors), and Iced's
/// Task::perform requires the message payload to be Clone.
async fn connect_and_list(
    cfg: AccountConfig,
    password: String,
) -> Result<Vec<FolderSummary>, String> {
    let backend = RustImapBackend::connect(&cfg, &password)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let folders = backend
        .list_folders()
        .await
        .map_err(|e| format!("list_folders: {e}"))?;
    backend.logout().await;
    Ok(folders)
}
