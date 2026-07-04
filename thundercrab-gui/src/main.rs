//! `thundercrab` — desktop GUI for the ThunderCrab mail client.
//!
//! A pure-Rust Iced app over the shared IMAP core. Boots into a connection
//! screen, then navigates folders → message list → a read view that renders
//! the message body with real formatting, searches a folder, saves attachments,
//! and performs message actions (flag / mark read / move / delete-to-Trash).
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
//! `Arc<RustImapBackend>` for the app's lifetime — every action reuses it (no
//! per-action reconnect). The backend serializes operations on its single socket
//! via an internal async `Mutex`; each async task clones the `Arc` before it
//! runs. Credentials stay in memory only and are never written to disk.
//!
//! ## Reading never mutates
//!
//! Opening a message uses `BODY.PEEK` and search uses `EXAMINE` — neither sets
//! `\Seen`. Read-state changes are explicit user actions (Mark read / unread).
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
    Column, button, column, container, markdown, row, scrollable, text, text_editor,
    text_input,
};
use iced::{Color, Element, Length, Task, Theme, theme::Palette};
use thundercrab_core::mail_html::markdown_to_safe_html;
use thundercrab_imap::{
    AccountConfig, Backend, FolderSummary, body::Attachment,
    rust_imap::{RustImapBackend, text_search_criterion},
    smtp::{OutboundAttachment, OutboundMessage, SmtpEncryption, send_message},
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

/// Which screen is on top. Connect → Folders → Messages → Reading, plus a
/// Compose screen reachable from Folders/Messages.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    #[default]
    Connect,
    Folders,
    Messages,
    Reading,
    Compose,
}

/// One staged outgoing attachment (owned bytes). Borrowed into an
/// [`OutboundAttachment`] at send time; kept owned in state so the compose list
/// survives across edits.
#[derive(Debug, Clone)]
struct ComposeAttachment {
    filename: String,
    mime_type: String,
    bytes: Vec<u8>,
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
/// which is not `Send` — on the main thread in `update`. Attachments (already
/// decoded to bytes by the core `fetch_body`) travel along so Save writes them
/// straight from memory with no second round-trip.
#[derive(Debug, Clone)]
struct LoadedBody {
    markdown: String,
    has_html: bool,
    attachments: Vec<Attachment>,
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
    /// `None` until Connect succeeds and after Logout.
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

    // Search within the open folder.
    search_query: String,
    searching: bool,
    /// True when the list currently shows search results (vs. the folder's
    /// recent headers) — drives the results label and empty-state copy.
    is_search_result: bool,

    // Read view.
    reading_uid: u32,
    reading_from: String,
    reading_subject: String,
    body: Option<markdown::Content>,
    body_note: String,
    loading_body: bool,
    /// Attachments of the open message (decoded bytes in memory), listed with
    /// a Save action each. Cleared when leaving the read view.
    reading_attachments: Vec<Attachment>,
    /// Whether the move-to-folder picker is expanded in the read view.
    show_move_picker: bool,

    // Compose. `compose_body` is a multi-line editor buffer (not `Send`, so it
    // lives here and never in a `Message`); `None` until a compose is started.
    to: String,
    cc: String,
    subject: String,
    compose_body: Option<text_editor::Content>,
    request_receipt: bool,
    compose_attachments: Vec<ComposeAttachment>,
    /// SMTP password captured just for the send; cleared immediately after.
    smtp_password: String,
    /// True while the send password prompt is showing.
    asking_send_password: bool,
    sending: bool,
    /// True while the "discard this draft?" confirmation is showing.
    confirm_discard: bool,
}

#[derive(Debug, Clone)]
enum Action {
    /// Set or clear an IMAP flag (`\\Seen`, `\\Flagged`) on the open message.
    Flag(&'static str, bool),
    /// Move the open message to another folder.
    Move(String),
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

    // --- Search ---
    SearchChanged(String),
    SubmitSearch,
    ClearSearch,
    SearchLoaded(Result<Vec<Row>, String>),

    // --- Attachments ---
    SaveAttachment(usize),
    AttachmentSaved(Result<String, String>),

    // --- Message actions ---
    /// Run an action on the currently-open message.
    DoAction(Action),
    /// Toggle the move-to-folder picker in the read view.
    ShowMovePicker(bool),
    /// Move the open message to the Trash special-use folder.
    DeleteCurrent,
    /// An action finished. The `bool` is `left_folder`: `true` for Move/Delete
    /// (the message left the open folder → return to the list and refresh),
    /// `false` for flag toggles (stay in the reader; nothing left the folder).
    ActionDone(Result<String, String>, bool),

    // --- Compose ---
    OpenCompose,
    ToChanged(String),
    CcChanged(String),
    SubjectChanged(String),
    ComposeBodyAction(text_editor::Action),
    ToggleReceipt(bool),
    PickComposeAttachment,
    /// A picked file was read (or the dialog was cancelled → `Ok(None)`), or an
    /// error (too large / unreadable).
    ComposeAttachmentPicked(Result<Option<ComposeAttachment>, String>),
    RemoveComposeAttachment(usize),
    /// Discard button in compose: confirms first if the draft has content.
    DiscardCompose,
    /// Confirm discarding the draft (leaves compose).
    ConfirmDiscard,
    /// Dismiss the discard confirmation and keep editing.
    KeepEditing,
    /// Show the "enter SMTP password" prompt (guarded on To + Subject).
    StartSend,
    SmtpPasswordChanged(String),
    CancelSend,
    ConfirmSend,
    Sent(Result<(), String>),
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

    /// The server's Trash mailbox, by `\Trash` special-use if marked, else by a
    /// case-insensitive name match. `None` if the account has no Trash folder.
    fn trash_folder(&self) -> Option<String> {
        self.folders
            .iter()
            .find(|f| f.special_use.as_deref() == Some("\\Trash"))
            .or_else(|| self.folders.iter().find(|f| f.name.eq_ignore_ascii_case("Trash")))
            .map(|f| f.name.clone())
    }

    /// Reload the open folder's recent headers on the shared session (used after
    /// an action that changed the folder's contents). No-op without a session.
    fn reload_folder(&mut self) -> Task<Message> {
        let Some(session) = self.session.clone() else {
            return Task::none();
        };
        self.is_search_result = false;
        // Abandon any in-flight search: its late SearchLoaded is then ignored
        // (see the `searching` guard there) so it can't clobber these headers.
        self.searching = false;
        self.loading_messages = true;
        Task::perform(list_messages(session, self.folder.clone()), Message::MessagesLoaded)
    }

    /// Whether the compose form holds anything worth confirming before discard.
    fn compose_dirty(&self) -> bool {
        self.compose_body.as_ref().is_some_and(|c| !c.text().trim().is_empty())
            || !self.to.trim().is_empty()
            || !self.cc.trim().is_empty()
            || !self.subject.trim().is_empty()
            || !self.compose_attachments.is_empty()
    }

    /// Leave the compose screen, clearing the draft, and return to wherever it
    /// was opened from (the message list if a folder is open, else Folders).
    fn leave_compose(&mut self) -> Task<Message> {
        self.compose_body = None;
        self.compose_attachments.clear();
        self.to.clear();
        self.cc.clear();
        self.subject.clear();
        self.confirm_discard = false;
        self.screen = if self.folder.is_empty() { Screen::Folders } else { Screen::Messages };
        Task::none()
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
                self.search_query.clear();
                self.is_search_result = false;
                self.searching = false;
                self.status.clear();
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
                self.reading_uid = uid;
                self.body = None;
                self.body_note.clear();
                self.reading_attachments.clear();
                self.show_move_picker = false;
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
                if !loaded.attachments.is_empty() {
                    notes.push(format!("{} attachment(s)", loaded.attachments.len()));
                }
                self.body_note = notes.join(" · ");
                self.reading_attachments = loaded.attachments;
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
                    // Discarding a compose returns to wherever it was opened
                    // from — the message list if a folder is open, else Folders.
                    Screen::Compose if !self.folder.is_empty() => Screen::Messages,
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
                self.reading_attachments.clear();
                self.status = "Signed out.".into();
                self.screen = Screen::Connect;
                Task::none()
            }
            Message::LinkClicked(url) => {
                // Privacy: never auto-open. Surface the destination instead.
                self.body_note = format!("Link (not opened): {url}");
                Task::none()
            }

            // --- Search ---
            Message::SearchChanged(q) => {
                self.search_query = q;
                Task::none()
            }
            Message::SubmitSearch => {
                let Some(session) = self.session.clone() else {
                    return Task::none();
                };
                let term = self.search_query.trim().to_string();
                if term.is_empty() {
                    // Empty query restores the folder's recent headers.
                    return self.reload_folder();
                }
                self.searching = true;
                Task::perform(
                    search_messages(session, self.folder.clone(), term),
                    Message::SearchLoaded,
                )
            }
            Message::ClearSearch => {
                self.search_query.clear();
                self.reload_folder()
            }
            Message::SearchLoaded(Ok(rows)) => {
                // Ignore a result whose search was abandoned (Clear / empty
                // submit / folder switch reset `searching`) — it must not
                // clobber the folder headers that replaced it.
                if !self.searching {
                    return Task::none();
                }
                self.searching = false;
                self.is_search_result = true;
                self.messages = rows;
                Task::none()
            }
            Message::SearchLoaded(Err(e)) => {
                if !self.searching {
                    return Task::none();
                }
                self.searching = false;
                self.status = format!("Search failed: {e}");
                Task::none()
            }

            // --- Attachments ---
            Message::SaveAttachment(index) => {
                let Some(att) = self.reading_attachments.get(index) else {
                    return Task::none();
                };
                let name = if att.filename.is_empty() {
                    format!("attachment-{index}")
                } else {
                    att.filename.clone()
                };
                let bytes = att.bytes.clone();
                Task::perform(save_attachment(name, bytes), Message::AttachmentSaved)
            }
            Message::AttachmentSaved(Ok(note)) => {
                self.body_note = note;
                Task::none()
            }
            Message::AttachmentSaved(Err(e)) => {
                self.body_note = format!("Save failed: {e}");
                Task::none()
            }

            // --- Message actions ---
            Message::ShowMovePicker(show) => {
                self.show_move_picker = show;
                Task::none()
            }
            Message::DoAction(action) => {
                let Some(session) = self.session.clone() else {
                    return Task::none();
                };
                self.show_move_picker = false;
                // Only Move removes the message from the open folder; flag toggles
                // leave it in place, so the reader should stay put.
                let left_folder = matches!(action, Action::Move(_));
                let (folder, uid) = (self.folder.clone(), self.reading_uid);
                Task::perform(run_action(session, folder, uid, action), move |r| {
                    Message::ActionDone(r, left_folder)
                })
            }
            Message::DeleteCurrent => {
                let Some(session) = self.session.clone() else {
                    return Task::none();
                };
                let Some(trash) = self.trash_folder() else {
                    self.body_note = "No Trash folder found for this account.".into();
                    return Task::none();
                };
                let (folder, uid) = (self.folder.clone(), self.reading_uid);
                Task::perform(
                    run_action(session, folder, uid, Action::Move(trash)),
                    |r| Message::ActionDone(r, true),
                )
            }
            Message::ActionDone(Ok(note), left_folder) => {
                if left_folder {
                    // The message left the folder — return to the list and refresh.
                    self.status = note;
                    self.screen = Screen::Messages;
                    self.reload_folder()
                } else {
                    // A flag toggle: stay in the reader; show the confirmation.
                    self.body_note = note;
                    Task::none()
                }
            }
            Message::ActionDone(Err(e), _) => {
                self.body_note = format!("Action failed: {e}");
                Task::none()
            }

            // --- Compose ---
            Message::OpenCompose => {
                self.to.clear();
                self.cc.clear();
                self.subject.clear();
                self.compose_body = Some(text_editor::Content::new());
                self.request_receipt = false;
                self.compose_attachments.clear();
                self.smtp_password.clear();
                self.asking_send_password = false;
                self.sending = false;
                self.confirm_discard = false;
                self.screen = Screen::Compose;
                Task::none()
            }
            Message::DiscardCompose => {
                if self.compose_dirty() {
                    self.confirm_discard = true;
                    Task::none()
                } else {
                    self.leave_compose()
                }
            }
            Message::ConfirmDiscard => {
                self.confirm_discard = false;
                self.leave_compose()
            }
            Message::KeepEditing => {
                self.confirm_discard = false;
                Task::none()
            }
            Message::ToChanged(s) => {
                self.to = s;
                Task::none()
            }
            Message::CcChanged(s) => {
                self.cc = s;
                Task::none()
            }
            Message::SubjectChanged(s) => {
                self.subject = s;
                Task::none()
            }
            Message::ComposeBodyAction(action) => {
                if let Some(content) = &mut self.compose_body {
                    content.perform(action);
                }
                Task::none()
            }
            Message::ToggleReceipt(on) => {
                self.request_receipt = on;
                Task::none()
            }
            Message::PickComposeAttachment => {
                Task::perform(pick_compose_attachment(), Message::ComposeAttachmentPicked)
            }
            Message::ComposeAttachmentPicked(Ok(Some(att))) => {
                self.compose_attachments.push(att);
                Task::none()
            }
            Message::ComposeAttachmentPicked(Ok(None)) => Task::none(),
            Message::ComposeAttachmentPicked(Err(e)) => {
                self.status = format!("Attach failed: {e}");
                Task::none()
            }
            Message::RemoveComposeAttachment(index) => {
                if index < self.compose_attachments.len() {
                    self.compose_attachments.remove(index);
                }
                Task::none()
            }
            Message::StartSend => {
                // Guard: need at least one recipient and a subject.
                if self.to.trim().is_empty() || self.subject.trim().is_empty() {
                    self.status = "To and Subject are required.".into();
                    return Task::none();
                }
                self.asking_send_password = true;
                Task::none()
            }
            Message::SmtpPasswordChanged(s) => {
                self.smtp_password = s;
                Task::none()
            }
            Message::CancelSend => {
                self.asking_send_password = false;
                self.smtp_password.clear();
                Task::none()
            }
            Message::ConfirmSend => {
                // Keep the prompt up (showing a disabled "Sending…") until the
                // send resolves; Sent(Ok/Err) dismisses it.
                self.sending = true;
                let cfg = self.account();
                let from = self.user.clone();
                let to = split_addrs(&self.to);
                let cc = split_addrs(&self.cc);
                let subject = self.subject.clone();
                let body = self.compose_body.as_ref().map(text_editor::Content::text).unwrap_or_default();
                let receipt = self.request_receipt;
                let attachments = self.compose_attachments.clone();
                // Move the captured password into the task and drop our copy.
                let password = std::mem::take(&mut self.smtp_password);
                Task::perform(
                    send(SendJob { cfg, password, from, to, cc, subject, body, receipt, attachments }),
                    Message::Sent,
                )
            }
            Message::Sent(Ok(())) => {
                self.sending = false;
                self.asking_send_password = false;
                self.status = "Message sent.".into();
                self.compose_body = None;
                self.compose_attachments.clear();
                self.to.clear();
                self.cc.clear();
                self.subject.clear();
                self.screen = if self.folder.is_empty() { Screen::Folders } else { Screen::Messages };
                Task::none()
            }
            Message::Sent(Err(e)) => {
                self.sending = false;
                // Dismiss the prompt back to the editable form so the error and
                // the retained draft are visible; the password was already
                // consumed, so a retry re-prompts.
                self.asking_send_password = false;
                self.status = format!("Send failed: {e}");
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
            Screen::Compose => self.compose_view(),
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
                button(text("Compose")).on_press(Message::OpenCompose),
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
            button(text("Compose")).on_press(Message::OpenCompose),
            self.theme_toggle(),
        ]
        .spacing(12);

        // Search bar: type a term and press Enter. Escaping happens in the core
        // (`text_search_criterion`); we pass the raw term only.
        let mut search_bar = row![
            text_input("Search this folder…", &self.search_query)
                .on_input(Message::SearchChanged)
                .on_submit(Message::SubmitSearch)
                .padding(8),
        ]
        .spacing(8);
        if self.is_search_result || !self.search_query.is_empty() {
            search_bar = search_bar.push(button(text("Clear")).on_press(Message::ClearSearch));
        }

        let count_note: Element<'_, Message> = if self.is_search_result && !self.searching {
            text(format!("{} result(s)", self.messages.len())).size(12).into()
        } else {
            container(text("")).into()
        };

        // Surface load/search errors and action confirmations, which the update
        // handlers write to `status` while this screen is showing.
        let status_note: Element<'_, Message> = if self.status.is_empty() {
            container(text("")).into()
        } else {
            text(&self.status).size(13).into()
        };

        let inner: Element<'_, Message> = if self.loading_messages || self.searching {
            text(if self.searching { "Searching…" } else { "Loading messages…" }).into()
        } else if self.messages.is_empty() {
            text(if self.is_search_result { "No matches." } else { "(no messages)" }).into()
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

        column![top, search_bar, status_note, count_note, inner].spacing(12).into()
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

        // Action bar. Reading itself never changed \Seen (BODY.PEEK), so read
        // state is an explicit choice here.
        let actions = row![
            button(text("Mark read").size(13))
                .on_press(Message::DoAction(Action::Flag("\\Seen", true))),
            button(text("Mark unread").size(13))
                .on_press(Message::DoAction(Action::Flag("\\Seen", false))),
            button(text("★ Flag").size(13))
                .on_press(Message::DoAction(Action::Flag("\\Flagged", true))),
            button(text("☆ Unflag").size(13))
                .on_press(Message::DoAction(Action::Flag("\\Flagged", false))),
            button(text("Move…").size(13))
                .on_press(Message::ShowMovePicker(!self.show_move_picker)),
            button(text("Delete").size(13)).on_press(Message::DeleteCurrent),
        ]
        .spacing(8);

        let mut col = column![top, head, actions].spacing(10);

        // Move picker: choose a destination folder (excludes the current one).
        if self.show_move_picker {
            let mut picker = Column::new().spacing(2);
            for f in &self.folders {
                if f.name == self.folder {
                    continue;
                }
                picker = picker.push(
                    button(text(f.name.clone()).size(13))
                        .width(Length::Fill)
                        .on_press(Message::DoAction(Action::Move(f.name.clone()))),
                );
            }
            col = col.push(
                column![
                    row![
                        text("Move to:").size(13).width(Length::Fill),
                        button(text("Cancel").size(13)).on_press(Message::ShowMovePicker(false)),
                    ]
                    .spacing(8),
                    scrollable(picker).height(Length::Fixed(160.0)),
                ]
                .spacing(4),
            );
        }

        if !self.body_note.is_empty() {
            col = col.push(text(&self.body_note).size(12));
        }

        // Attachment list with a Save action each.
        if !self.reading_attachments.is_empty() {
            let mut atts = Column::new().spacing(4);
            for (i, a) in self.reading_attachments.iter().enumerate() {
                let name = if a.filename.is_empty() { "(unnamed)" } else { a.filename.as_str() };
                let label = format!("{name} · {} · {}", a.mime_type, human_size(a.size()));
                atts = atts.push(
                    row![
                        text(label).size(13).width(Length::Fill),
                        button(text("Save").size(13)).on_press(Message::SaveAttachment(i)),
                    ]
                    .spacing(8),
                );
            }
            col = col.push(atts);
        }

        let body: Element<'_, Message> = if self.loading_body {
            text("Loading message…").into()
        } else if let Some(content) = &self.body {
            let th = active_theme(self.dark);
            markdown::view(content.items(), &th).map(Message::LinkClicked)
        } else {
            text("(no body)").into()
        };

        col = col.push(scrollable(body).height(Length::Fill));
        col.spacing(10).into()
    }

    fn compose_view(&self) -> Element<'_, Message> {
        let top = row![
            button(text("← Discard")).on_press(Message::DiscardCompose),
            text("New message").size(22).width(Length::Fill),
            self.theme_toggle(),
        ]
        .spacing(12);

        // Discard confirmation: only reached when the draft has content.
        if self.confirm_discard {
            let confirm = column![
                text("Discard this draft?").size(15),
                text("Your message and attachments will be lost.").size(12),
                row![
                    button(text("Discard")).on_press(Message::ConfirmDiscard),
                    button(text("Keep editing")).on_press(Message::KeepEditing),
                ]
                .spacing(12),
            ]
            .spacing(10);
            return column![top, confirm].spacing(16).into();
        }

        // Password prompt: shown when the user hits Send, and kept up (showing a
        // disabled "Sending…") while the send is in flight. Inline (iced has no
        // modal); the password is used once and never stored.
        if self.asking_send_password || self.sending {
            let prompt = column![
                text("Enter your mail password to send.").size(15),
                text("It is used once for this send and never stored.").size(12),
                secure_field("Password", &self.smtp_password, Message::SmtpPasswordChanged),
                row![
                    button(text(if self.sending { "Sending…" } else { "Send" })).on_press_maybe(
                        (!self.smtp_password.is_empty() && !self.sending)
                            .then_some(Message::ConfirmSend),
                    ),
                    // Cancel is unavailable mid-send (the request can't be recalled).
                    button(text("Cancel"))
                        .on_press_maybe((!self.sending).then_some(Message::CancelSend)),
                ]
                .spacing(12),
            ]
            .spacing(10);
            return column![top, prompt].spacing(16).into();
        }

        let fields = column![
            field("To", "someone@example.com", &self.to, Message::ToChanged),
            field("Cc (optional)", "", &self.cc, Message::CcChanged),
            field("Subject", "", &self.subject, Message::SubjectChanged),
        ]
        .spacing(8);

        // Multi-line Markdown body via the text editor buffer.
        let body_editor: Element<'_, Message> = self.compose_body.as_ref().map_or_else(
            || text("(compose buffer not ready)").into(),
            |content| {
                text_editor(content)
                    .placeholder("Write your message (Markdown supported)…")
                    .on_action(Message::ComposeBodyAction)
                    .height(Length::Fixed(240.0))
                    .into()
            },
        );

        // Attachment picker + staged chips (removable).
        let mut attach_col = column![
            row![
                button(text("Attach file")).on_press(Message::PickComposeAttachment),
                text("Max 25 MB each").size(12),
            ]
            .spacing(12),
        ]
        .spacing(6);
        for (i, a) in self.compose_attachments.iter().enumerate() {
            attach_col = attach_col.push(
                row![
                    text(format!("{} · {}", a.filename, human_size(a.bytes.len())))
                        .size(13)
                        .width(Length::Fill),
                    button(text("✕").size(13)).on_press(Message::RemoveComposeAttachment(i)),
                ]
                .spacing(8),
            );
        }

        let receipt_row = row![
            text("Request read receipt").size(14).width(Length::Fill),
            iced::widget::checkbox(self.request_receipt).on_toggle(Message::ToggleReceipt),
        ]
        .spacing(8);

        let can_send = !self.to.trim().is_empty() && !self.subject.trim().is_empty() && !self.sending;
        let send_row = row![
            button(text("Send")).on_press_maybe(can_send.then_some(Message::StartSend)),
            text(&self.status).size(12),
        ]
        .spacing(12);

        let form = column![
            fields,
            body_editor,
            attach_col,
            receipt_row,
            send_row,
        ]
        .spacing(14);

        column![top, scrollable(form).height(Length::Fill)].spacing(16).into()
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

/// Human-readable byte size (B / KB / MB) for the attachment list.
fn human_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    #[allow(clippy::cast_precision_loss)]
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
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

/// Async: search `folder` for `term` on the shared session. The raw term is
/// turned into a safe IMAP `TEXT "…"` criterion by the core
/// `text_search_criterion` (escaping stays in Rust; the UI never builds IMAP
/// syntax). `EXAMINE` — never marks `\Seen`.
async fn search_messages(
    session: Arc<RustImapBackend>,
    folder: String,
    term: String,
) -> Result<Vec<Row>, String> {
    let criterion = text_search_criterion(&term);
    let headers = session
        .search(&folder, &criterion)
        .await
        .map_err(|e| format!("search: {e}"))?;
    Ok(headers
        .into_iter()
        .map(|h| Row { uid: h.uid, from: h.from, subject: h.subject })
        .collect())
}

/// Async: fetch one body on the shared session. Prefers the plain-text part as
/// the markdown source (faithful for ThunderCrab-composed mail and for
/// mail-parser's HTML→text rendering); reports whether a richer HTML part
/// exists and carries any decoded attachments. `BODY.PEEK` — reading never sets
/// `\Seen`.
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
        attachments: body.attachments,
    })
}

/// Async: prompt for a destination via a native file dialog, then write the
/// attachment bytes there. Returns a human status; a cancelled dialog is a
/// benign non-error. The bytes are already in memory (from `fetch_body`), so no
/// network round-trip happens here.
async fn save_attachment(filename: String, bytes: Vec<u8>) -> Result<String, String> {
    let handle = rfd::AsyncFileDialog::new()
        .set_file_name(&filename)
        .save_file()
        .await;
    let Some(handle) = handle else {
        return Ok("Save cancelled.".to_string());
    };
    let path = handle.path().to_path_buf();
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(format!("Saved to {}", path.display()))
}

/// Async: run a message [`Action`] on the shared session and report a status.
async fn run_action(
    session: Arc<RustImapBackend>,
    folder: String,
    uid: u32,
    action: Action,
) -> Result<String, String> {
    match action {
        Action::Flag(flag, set) => {
            session
                .set_flag(&folder, uid, flag, set)
                .await
                .map_err(|e| format!("set_flag: {e}"))?;
            Ok(format!(
                "{} {}",
                if set { "Set" } else { "Cleared" },
                flag.trim_start_matches('\\')
            ))
        }
        Action::Move(to) => {
            session
                .move_message(&folder, &to, uid)
                .await
                .map_err(|e| format!("move_message: {e}"))?;
            Ok(format!("Moved to {to}"))
        }
    }
}

/// Cap on how large a picked file we'll read into memory for a compose
/// attachment (25 MB) — guards against OOM and mirrors the Android limit; most
/// mail servers reject larger messages anyway.
const MAX_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;

/// Split a raw recipient string on commas / semicolons / whitespace into
/// trimmed, non-empty addresses.
fn split_addrs(raw: &str) -> Vec<String> {
    raw.split([',', ';', '\n', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Best-effort MIME type from a file extension. The SMTP layer falls back to
/// `application/octet-stream` for anything it can't parse, so an unknown
/// extension here is harmless.
fn mime_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("html" | "htm") => "text/html",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        Some("doc") => "application/msword",
        Some("docx") => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        _ => "application/octet-stream",
    }
}

/// Async: prompt for a file to attach, read it, enforce the size cap, and infer
/// its MIME type. `Ok(None)` means the dialog was cancelled (benign).
async fn pick_compose_attachment() -> Result<Option<ComposeAttachment>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new().pick_file().await else {
        return Ok(None);
    };
    let path = handle.path().to_path_buf();
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(format!("{} is larger than 25 MB", path.display()));
    }
    let filename = path.file_name().map_or_else(
        || "attachment".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mime_type = mime_for_path(&path).to_string();
    Ok(Some(ComposeAttachment { filename, mime_type, bytes }))
}

/// Everything one send needs, owned. Bundled into a struct so the send fn takes
/// a single argument (not nine).
struct SendJob {
    cfg: AccountConfig,
    password: String,
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    receipt: bool,
    attachments: Vec<ComposeAttachment>,
}

/// Async: build and send one message over STARTTLS (587). The Markdown source
/// travels as the text/plain part; its sanitized HTML rendering is the
/// alternative (so HTML clients show formatting and others fall back). A blank
/// body sends no HTML part. `read_receipt_to` is the sender's own address when
/// requested. The password is used here and dropped when the task ends.
async fn send(job: SendJob) -> Result<(), String> {
    let to_refs: Vec<&str> = job.to.iter().map(String::as_str).collect();
    let cc_refs: Vec<&str> = job.cc.iter().map(String::as_str).collect();
    let html = markdown_to_safe_html(&job.body);
    let att_views: Vec<OutboundAttachment> = job
        .attachments
        .iter()
        .map(|a| OutboundAttachment {
            filename: &a.filename,
            mime_type: &a.mime_type,
            bytes: &a.bytes,
        })
        .collect();
    let msg = OutboundMessage {
        from: &job.from,
        to: &to_refs,
        cc: &cc_refs,
        subject: &job.subject,
        body: &job.body,
        html_body: if job.body.trim().is_empty() { None } else { Some(html.as_str()) },
        read_receipt_to: job.receipt.then_some(job.from.as_str()),
        attachments: &att_views,
    };
    send_message(&job.cfg, &job.password, SmtpEncryption::StartTls, &msg)
        .await
        .map_err(|e| format!("send: {e}"))
}
