//! `thundercrab` — desktop GUI for the ThunderCrab mail client.
//!
//! A pure-Rust Iced app over the shared IMAP core. Boots into a connection
//! screen, then navigates folders → message list → a read view that renders
//! the message body with real formatting.
//!
//! ## Why Iced
//!
//! Toolkit chosen 2026-04-27 from four candidates (GTK4, Iced, Tauri, Slint).
//! The decision criteria were the supersociety standard — pure Rust, single
//! binary per platform, compile-time-typed state, **no embedded WebView** or
//! system GUI runtime.
//!
//! ## HTML / formatted mail WITHOUT a WebView
//!
//! The read view renders the message body through Iced's native `markdown`
//! widget (CommonMark → real Iced widgets). We feed it the message's plain-text
//! part: for mail composed in ThunderCrab that part IS the original Markdown
//! (so it renders with full formatting), and for HTML-only senders `mail-parser`
//! synthesizes a faithful text rendering. No HTML engine, no network, no WebView
//! — consistent with the no-WebView pick while still showing formatted mail.
//!
//! ## What this does today
//!
//! Connect → list folders → open a folder → list recent message headers → open
//! a message and read its (formatted) body, with a note when a richer HTML part
//! or attachments exist. Each action reconnects, runs, and logs out (the app
//! holds no long-lived IMAP session yet); credentials stay in memory only.
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
    Column, button, column, container, markdown, row, scrollable, text, text_input,
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

/// Which screen is on top. Connect → Folders → Messages → Reading.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    #[default]
    Connect,
    Folders,
    Messages,
    Reading,
}

/// A one-line message summary for the list (header-only).
#[derive(Debug, Clone)]
struct Row {
    uid: u32,
    from: String,
    subject: String,
}

/// A loaded body, ready to display. The markdown source is carried as a String
/// (Iced messages must be `Send`); `App` parses it into `markdown::Content` —
/// which is not `Send` — on the main thread in `update`.
#[derive(Debug, Clone)]
struct LoadedBody {
    markdown: String,
    has_html: bool,
    attachments: usize,
}

/// App state. Single struct; transitions are pure functions of
/// (state, message) → (state, command). Credentials stay in memory only and
/// are never written to disk.
#[derive(Default)]
struct App {
    // Connection form (host/user/password are retained after connect so each
    // action can reconnect — the app holds no long-lived IMAP session yet).
    host: String,
    user: String,
    password: String,
    status: String,
    connecting: bool,
    connected: bool,

    screen: Screen,

    // Folder list, populated after a successful connect.
    folders: Vec<FolderSummary>,

    // Message list for the open folder.
    folder: String,
    messages: Vec<Row>,
    loading_messages: bool,

    // Read view.
    reading_from: String,
    reading_subject: String,
    body: Option<markdown::Content>,
    body_note: String,
    loading_body: bool,
}

#[derive(Debug, Clone)]
enum Message {
    HostChanged(String),
    UserChanged(String),
    PasswordChanged(String),
    Connect,
    Connected(Result<Vec<FolderSummary>, String>),
    OpenFolder(String),
    MessagesLoaded(Result<Vec<Row>, String>),
    OpenMessage(u32),
    BodyLoaded(Result<LoadedBody, String>),
    Back,
    LinkClicked(markdown::Uri),
}

impl App {
    fn account(&self) -> AccountConfig {
        AccountConfig::plausiden(self.host.clone(), self.user.clone())
    }

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
                let (cfg, pw) = (self.account(), self.password.clone());
                Task::perform(connect_and_list(cfg, pw), Message::Connected)
            }
            Message::Connected(Ok(folders)) => {
                self.connecting = false;
                self.connected = true;
                self.folders = folders;
                self.status = format!("Connected. {} folders.", self.folders.len());
                self.screen = Screen::Folders;
                Task::none()
            }
            Message::Connected(Err(e)) => {
                self.connecting = false;
                self.folders.clear();
                self.status = format!("Failed: {e}");
                Task::none()
            }
            Message::OpenFolder(name) => {
                self.folder = name.clone();
                self.messages.clear();
                self.loading_messages = true;
                self.screen = Screen::Messages;
                let (cfg, pw) = (self.account(), self.password.clone());
                Task::perform(list_messages(cfg, pw, name), Message::MessagesLoaded)
            }
            Message::MessagesLoaded(Ok(rows)) => {
                self.loading_messages = false;
                self.messages = rows;
                Task::none()
            }
            Message::MessagesLoaded(Err(e)) => {
                self.loading_messages = false;
                self.status = format!("Failed to load {}: {e}", self.folder);
                Task::none()
            }
            Message::OpenMessage(uid) => {
                if let Some(r) = self.messages.iter().find(|r| r.uid == uid) {
                    self.reading_from = r.from.clone();
                    self.reading_subject = r.subject.clone();
                }
                self.body = None;
                self.body_note.clear();
                self.loading_body = true;
                self.screen = Screen::Reading;
                let (cfg, pw, folder) = (self.account(), self.password.clone(), self.folder.clone());
                Task::perform(load_body(cfg, pw, folder, uid), Message::BodyLoaded)
            }
            Message::BodyLoaded(Ok(loaded)) => {
                self.loading_body = false;
                self.body = Some(markdown::Content::parse(&loaded.markdown));
                let mut notes = Vec::new();
                if loaded.has_html {
                    notes.push("a richer HTML part exists".to_string());
                }
                if loaded.attachments > 0 {
                    notes.push(format!("{} attachment(s)", loaded.attachments));
                }
                self.body_note = notes.join(" · ");
                Task::none()
            }
            Message::BodyLoaded(Err(e)) => {
                self.loading_body = false;
                self.body_note = format!("Couldn't load the body: {e}");
                Task::none()
            }
            Message::Back => {
                self.screen = match self.screen {
                    Screen::Reading => Screen::Messages,
                    _ => Screen::Folders,
                };
                Task::none()
            }
            Message::LinkClicked(url) => {
                // Privacy: never auto-open. Surface the destination instead.
                self.body_note = format!("Link (not opened): {url}");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let body = match self.screen {
            Screen::Connect => self.connect_view(),
            Screen::Folders => self.folders_view(),
            Screen::Messages => self.messages_view(),
            Screen::Reading => self.reading_view(),
        };
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(24)
            .into()
    }

    fn connect_view(&self) -> Element<'_, Message> {
        let header = column![
            text("ThunderCrab").size(32),
            text("Local-first mail client. Rules transparent. No cloud.").size(14),
        ]
        .spacing(4);

        let form = column![
            field("IMAP host", "mail.example.com", &self.host, Message::HostChanged),
            field("Username", "you@example.com", &self.user, Message::UserChanged),
            secure_field("Password", &self.password, Message::PasswordChanged),
            row![
                button(text(if self.connecting { "Connecting…" } else { "Connect" }))
                    .on_press_maybe((!self.connecting).then_some(Message::Connect)),
                text(&self.status).size(13),
            ]
            .spacing(12),
        ]
        .spacing(8);

        column![header, form].spacing(24).into()
    }

    fn folders_view(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(4);
        for f in &self.folders {
            let label = format!("{:>4} / {:<5}  {}", f.unseen, f.messages, f.name);
            list = list.push(
                button(text(label))
                    .width(Length::Fill)
                    .on_press(Message::OpenFolder(f.name.clone())),
            );
        }
        column![
            text("Folders").size(24),
            text(&self.status).size(13),
            scrollable(list).height(Length::Fill),
        ]
        .spacing(12)
        .into()
    }

    fn messages_view(&self) -> Element<'_, Message> {
        let top = row![
            button(text("← Folders")).on_press(Message::Back),
            text(prettify(&self.folder)).size(22),
        ]
        .spacing(12);

        let inner: Element<'_, Message> = if self.loading_messages {
            text("Loading messages…").into()
        } else if self.messages.is_empty() {
            text("(no messages)").into()
        } else {
            let mut list = Column::new().spacing(2);
            for m in &self.messages {
                let row_label = column![
                    text(m.from.clone()).size(15),
                    text(m.subject.clone()).size(13),
                ]
                .spacing(1);
                list = list.push(
                    button(row_label)
                        .width(Length::Fill)
                        .on_press(Message::OpenMessage(m.uid)),
                );
            }
            scrollable(list).height(Length::Fill).into()
        };

        column![top, inner].spacing(12).into()
    }

    fn reading_view(&self) -> Element<'_, Message> {
        let top = row![button(text("← Back")).on_press(Message::Back)].spacing(12);

        let head = column![
            text(non_blank(&self.reading_subject, "(no subject)")).size(22),
            text(non_blank(&self.reading_from, "(unknown sender)")).size(14),
        ]
        .spacing(2);

        let body: Element<'_, Message> = if self.loading_body {
            text("Loading message…").into()
        } else if let Some(content) = &self.body {
            markdown::view(content.items(), &Theme::Light).map(Message::LinkClicked)
        } else {
            text("(no body)").into()
        };

        let mut col = column![top, head].spacing(10);
        if !self.body_note.is_empty() {
            col = col.push(text(&self.body_note).size(12));
        }
        col = col.push(scrollable(body).height(Length::Fill));
        col.spacing(10).into()
    }
}

/// A labeled single-line text input row.
fn field<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::Fixed(120.0)),
        text_input(placeholder, value).on_input(on_input).padding(8),
    ]
    .spacing(8)
    .into()
}

/// A labeled password (obscured) input row.
fn secure_field<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::Fixed(120.0)),
        text_input("…", value).secure(true).on_input(on_input).padding(8),
    ]
    .spacing(8)
    .into()
}

/// Strip the IMAP hierarchy prefix for a friendlier title (Archive/2026 → 2026).
fn prettify(raw: &str) -> String {
    raw.rsplit('/').next().unwrap_or(raw).to_string()
}

fn non_blank<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    if s.trim().is_empty() { fallback } else { s }
}

/// Async: connect, list folders. Error type is String because BackendError
/// isn't Clone (transport variants own non-clonable inner errors) and Iced's
/// Task::perform requires a Clone payload.
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

/// Async: connect, fetch the most recent headers in `folder`, log out.
async fn list_messages(
    cfg: AccountConfig,
    password: String,
    folder: String,
) -> Result<Vec<Row>, String> {
    let backend = RustImapBackend::connect(&cfg, &password)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let headers = backend
        .fetch_headers(&folder, Some(50))
        .await
        .map_err(|e| format!("fetch_headers: {e}"))?;
    backend.logout().await;
    Ok(headers
        .into_iter()
        .map(|h| Row { uid: h.uid, from: h.from, subject: h.subject })
        .collect())
}

/// Async: connect, fetch one body, log out. Prefers the plain-text part as the
/// markdown source (faithful for ThunderCrab-composed mail and for mail-parser's
/// HTML→text rendering); reports whether a richer HTML part / attachments exist.
async fn load_body(
    cfg: AccountConfig,
    password: String,
    folder: String,
    uid: u32,
) -> Result<LoadedBody, String> {
    let backend = RustImapBackend::connect(&cfg, &password)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let body = backend
        .fetch_body(&folder, uid)
        .await
        .map_err(|e| format!("fetch_body: {e}"))?;
    backend.logout().await;
    let markdown = body
        .plain
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "_(no text body)_".to_string());
    Ok(LoadedBody {
        markdown,
        has_html: body.html_sanitized.is_some(),
        attachments: body.attachments.len(),
    })
}
