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
//! ## Session model
//!
//! One authenticated IMAP session is opened on Connect and held in state as an
//! `Arc<RustImapBackend>` for the app's lifetime — folder/message/read actions
//! reuse it (no per-action reconnect). The backend serializes operations on its
//! single socket via an internal async `Mutex`; each async task clones the `Arc`
//! before it runs. Credentials stay in memory only and are never written to disk.
//!
//! Command to run:
//!
//! ```bash
//! cargo run -p thundercrab-gui
//! ```

#![doc(html_no_source)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]

use std::sync::{Arc, LazyLock};

use iced::widget::{
    Column, button, column, container, markdown, row, scrollable, text, text_input,
};
use iced::{Color, Element, Length, Task, Theme, theme::Palette};
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

    iced::application(App::boot, App::update, App::view)
        .title("ThunderCrab")
        .theme(theme)
        .run()
}

/// AMOLED true-black dark theme (default). Built once. `background = #000000`
/// so OLED panels draw the canvas with zero lit pixels; `text` is a soft
/// off-white for contrast without glare. The accent palette matches the
/// ThunderCrab brand blurple. iced's [`Palette`] has exactly six colour slots.
static AMOLED: LazyLock<Theme> = LazyLock::new(|| {
    Theme::custom(
        "AMOLED".to_string(),
        Palette {
            background: Color::BLACK,
            text: Color::from_rgb8(0xE6, 0xE6, 0xE6),
            primary: Color::from_rgb8(0x58, 0x65, 0xF2),
            success: Color::from_rgb8(0x3B, 0xA5, 0x5D),
            warning: Color::from_rgb8(0xFA, 0xA6, 0x1A),
            danger: Color::from_rgb8(0xED, 0x42, 0x45),
        },
    )
});

/// The active theme as a value. AMOLED when dark (the default), else iced's
/// built-in Light. Returned by value so callers can lend a short-lived `&Theme`
/// to widgets like `markdown::view` (which copies what it needs and does not
/// retain the borrow).
fn active_theme(dark: bool) -> Theme {
    if dark { AMOLED.clone() } else { Theme::Light }
}

/// Application theme hook. A named fn (not a closure) so the `&App` lifetime is
/// universally quantified — a closure here infers one specific lifetime and
/// trips iced's `for<'a>` theme bound ("implementation of `Fn` is not general
/// enough").
fn theme(state: &App) -> Theme {
    active_theme(state.dark)
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
// A reader/compose UI legitimately tracks several independent async-in-flight
// and mode flags (connecting, loading, searching, sending, theme). Modelling
// each as a two-variant enum would add noise, not clarity.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct App {
    // Connection form. host/user/password are retained after connect only so a
    // reconnect (e.g. after Logout) can be attempted; the live session below is
    // what actual operations use.
    host: String,
    user: String,
    password: String,
    status: String,
    connecting: bool,
    connected: bool,

    /// The one long-lived authenticated IMAP session, shared by every action.
    /// `None` until Connect succeeds and after Logout. Never placed in a
    /// `Message` except as the freshly-opened handle from `open_session`.
    session: Option<Arc<RustImapBackend>>,

    /// Theme flag. `true` = AMOLED dark (default), `false` = Light.
    dark: bool,

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

// `OpenFolder`/`OpenMessage` share an "Open" prefix by intent — they are the
// two navigation-drill actions and read best paired.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
enum Message {
    HostChanged(String),
    UserChanged(String),
    PasswordChanged(String),
    Connect,
    /// The opened session plus its initial folder list, or a failure string.
    /// `Arc<RustImapBackend>` is `Clone` (Arc) and `Debug` (the backend derives
    /// it), so it rides in a `Message` with no wrapper needed.
    Connected(Result<(Arc<RustImapBackend>, Vec<FolderSummary>), String>),
    OpenFolder(String),
    MessagesLoaded(Result<Vec<Row>, String>),
    OpenMessage(u32),
    BodyLoaded(Result<LoadedBody, String>),
    Back,
    ToggleTheme,
    Logout,
    LinkClicked(markdown::Uri),
}

impl App {
    /// Boot state: everything default except the theme, which starts dark
    /// (AMOLED) per the house dark-first preference.
    fn boot() -> Self {
        Self { dark: true, ..Self::default() }
    }

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
                Task::perform(open_session(cfg, pw), Message::Connected)
            }
            Message::Connected(Ok((session, folders))) => {
                self.connecting = false;
                self.connected = true;
                self.session = Some(session);
                self.folders = folders;
                self.status = format!("Connected. {} folders.", self.folders.len());
                self.screen = Screen::Folders;
                Task::none()
            }
            Message::Connected(Err(e)) => {
                self.connecting = false;
                self.session = None;
                self.folders.clear();
                self.status = format!("Failed: {e}");
                Task::none()
            }
            Message::OpenFolder(name) => {
                let Some(session) = self.session.clone() else {
                    self.status = "Not connected.".into();
                    return Task::none();
                };
                self.folder.clone_from(&name);
                self.messages.clear();
                self.loading_messages = true;
                self.screen = Screen::Messages;
                Task::perform(list_messages(session, name), Message::MessagesLoaded)
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
                let Some(session) = self.session.clone() else {
                    self.status = "Not connected.".into();
                    return Task::none();
                };
                if let Some(r) = self.messages.iter().find(|r| r.uid == uid) {
                    self.reading_from = r.from.clone();
                    self.reading_subject = r.subject.clone();
                }
                self.body = None;
                self.body_note.clear();
                self.loading_body = true;
                self.screen = Screen::Reading;
                let folder = self.folder.clone();
                Task::perform(load_body(session, folder, uid), Message::BodyLoaded)
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
            Message::ToggleTheme => {
                self.dark = !self.dark;
                Task::none()
            }
            Message::Logout => {
                // Drop the session handle: the last `Arc` owner closing the TLS
                // stream tears down the connection. A clean IMAP `LOGOUT` would
                // need owning the backend (`logout(self)`), which we can't while
                // it may be shared with an in-flight task — dropping is the
                // correct, race-free teardown here.
                self.session = None;
                self.connected = false;
                self.folders.clear();
                self.messages.clear();
                self.body = None;
                self.body_note.clear();
                self.status = "Signed out.".into();
                self.screen = Screen::Connect;
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
        let header = row![
            column![
                text("ThunderCrab").size(32),
                text("Local-first mail client. Rules transparent. No cloud.").size(14),
            ]
            .spacing(4)
            .width(Length::Fill),
            self.theme_toggle(),
        ]
        .spacing(12);

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
            row![
                text("Folders").size(24).width(Length::Fill),
                button(text("Sign out")).on_press(Message::Logout),
                self.theme_toggle(),
            ]
            .spacing(12),
            text(&self.status).size(13),
            scrollable(list).height(Length::Fill),
        ]
        .spacing(12)
        .into()
    }

    fn messages_view(&self) -> Element<'_, Message> {
        let top = row![
            button(text("← Folders")).on_press(Message::Back),
            text(prettify(&self.folder)).size(22).width(Length::Fill),
            self.theme_toggle(),
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
        let top = row![
            button(text("← Back")).on_press(Message::Back),
            container(text("")).width(Length::Fill),
            self.theme_toggle(),
        ]
        .spacing(12);

        let head = column![
            text(non_blank(&self.reading_subject, "(no subject)")).size(22),
            text(non_blank(&self.reading_from, "(unknown sender)")).size(14),
        ]
        .spacing(2);

        let body: Element<'_, Message> = if self.loading_body {
            text("Loading message…").into()
        } else if let Some(content) = &self.body {
            let th = active_theme(self.dark);
            markdown::view(content.items(), &th).map(Message::LinkClicked)
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

    /// A compact theme-toggle button, shown in every screen's top bar.
    fn theme_toggle(&self) -> Element<'_, Message> {
        let label = if self.dark { "☀ Light" } else { "☾ Dark" };
        button(text(label).size(13)).on_press(Message::ToggleTheme).into()
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

/// Async: open one authenticated session and list its folders. The returned
/// `Arc<RustImapBackend>` becomes the app's long-lived session. Error type is
/// `String` because `BackendError` isn't `Clone` (transport variants own
/// non-clonable inner errors) and Iced's `Task::perform` requires a `Clone`
/// payload.
async fn open_session(
    cfg: AccountConfig,
    password: String,
) -> Result<(Arc<RustImapBackend>, Vec<FolderSummary>), String> {
    let backend = RustImapBackend::connect(&cfg, &password)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let folders = backend
        .list_folders()
        .await
        .map_err(|e| format!("list_folders: {e}"))?;
    Ok((Arc::new(backend), folders))
}

/// Async: fetch the most recent headers in `folder` on the shared session.
async fn list_messages(
    session: Arc<RustImapBackend>,
    folder: String,
) -> Result<Vec<Row>, String> {
    let headers = session
        .fetch_headers(&folder, Some(50))
        .await
        .map_err(|e| format!("fetch_headers: {e}"))?;
    Ok(headers
        .into_iter()
        .map(|h| Row { uid: h.uid, from: h.from, subject: h.subject })
        .collect())
}

/// Async: fetch one body on the shared session. Prefers the plain-text part as
/// the markdown source (faithful for ThunderCrab-composed mail and for
/// mail-parser's HTML→text rendering); reports whether a richer HTML part /
/// attachments exist. `BODY.PEEK` — reading never sets `\Seen`.
async fn load_body(
    session: Arc<RustImapBackend>,
    folder: String,
    uid: u32,
) -> Result<LoadedBody, String> {
    let body = session
        .fetch_body(&folder, uid)
        .await
        .map_err(|e| format!("fetch_body: {e}"))?;
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
