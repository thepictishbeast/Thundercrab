//! `ManageSieve` client (RFC 5804) — push Sieve scripts to the server.
//!
//! `ManageSieve` is a small, line-oriented protocol that runs on port
//! 4190 with a STARTTLS upgrade. It's the standardized mechanism for
//! a mail client to install or replace the user's server-side
//! filtering rules. ThunderCrab uses it as the persistence path for
//! the typed `CrabRule` chain — generate Sieve, push it, set it active,
//! the server applies the rules to incoming mail.
//!
//! ## Why we hand-roll this
//!
//! The protocol is small enough (~10 commands, line-based with IMAP-
//! style `{N}\r\n` literals) that pulling a third-party crate would
//! cost more in vendoring + audit than writing it directly. The
//! implementation lives in this single file; the parser is one
//! state machine with three states (line, literal-length, literal-
//! body); the TLS upgrade is the same pattern as our IMAP backend.
//!
//! ## Public surface
//!
//! Only one entry point is exposed: [`put_active_script`]. It runs
//! the entire flow (connect → greeting → STARTTLS → AUTHENTICATE →
//! PUTSCRIPT → SETACTIVE → LOGOUT) in a single call and returns
//! when the server has accepted the script. The Backend trait
//! method `put_sieve` calls through to this.
//!
//! ## Limitations
//!
//! * Only SASL PLAIN is supported. Most `ManageSieve` servers also
//!   accept LOGIN; the difference doesn't matter once you're inside
//!   TLS, but if a server refuses PLAIN we surface that as Auth.
//! * No script-listing, no GETSCRIPT, no DELETESCRIPT. ThunderCrab's
//!   model is single-script (the typed `CrabRule` chain owns the whole
//!   filter); we PUT-and-SETACTIVE in one call, never read back.
//! * No HAVESPACE preflight. Servers that reject oversize scripts
//!   surface as Protocol; the GUI re-prompts.

use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::ServerName;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::{AccountConfig, BackendError};

/// One-shot: connect, upgrade, authenticate, install the script,
/// set it active, log out.
///
/// `script_name` is the server-side identifier (e.g., `"thundercrab"`).
/// `script` is the Sieve source. Empty `script` is permitted by
/// the protocol but ThunderCrab callers should pass a non-empty
/// string — clearing rules is a separate, deliberate operation.
///
/// # Errors
/// - `Transport` for TCP / TLS handshake / I/O failures.
/// - `Auth` if SASL AUTHENTICATE PLAIN is rejected.
/// - `Protocol` for any NO/BYE response from the server.
pub async fn put_active_script(
    cfg: &AccountConfig,
    password: &str,
    script_name: &str,
    script: &str,
) -> Result<(), BackendError> {
    let host = cfg.imap_host.as_str();
    let port = cfg.sieve_port;

    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| BackendError::Transport(format!("tcp connect: {e}")))?;
    let mut stream = BufReader::new(tcp);

    // Greeting: capabilities then OK. We ignore capability content
    // for v0 — the server we target advertises STARTTLS, PLAIN, and
    // the protocol version we support, and a misbehaving server will
    // surface as Protocol when its actual response disagrees with
    // what we send next.
    read_until_ok(&mut stream).await?;

    // STARTTLS upgrade. Send the verb, read OK, then wrap.
    stream.get_mut().write_all(b"STARTTLS\r\n").await
        .map_err(|e| BackendError::Transport(format!("STARTTLS write: {e}")))?;
    read_until_ok(&mut stream).await?;
    let tcp = stream.into_inner();

    let connector = TlsConnector::from(Arc::new(tls_config()));
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| BackendError::Transport(format!("server name: {e}")))?;
    let tls = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| BackendError::Transport(format!("tls handshake: {e}")))?;
    let mut tls = BufReader::new(tls);

    // After TLS upgrade the server re-sends its capabilities + OK.
    read_until_ok(&mut tls).await?;

    // SASL PLAIN: NUL <authorization-id> NUL <authentication-id> NUL
    // <password>. We use an empty authorization-id (use the same
    // identity as authentication-id, which is the standard pattern).
    let auth_blob = format!("\0{}\0{}", cfg.username, password);
    let auth_b64 = base64_encode(auth_blob.as_bytes());
    write_command(
        tls.get_mut(),
        &format!("AUTHENTICATE \"PLAIN\" \"{auth_b64}\""),
    )
    .await?;
    match read_response(&mut tls).await? {
        Response::Ok => {}
        Response::No(reason) => {
            return Err(BackendError::Auth(reason));
        }
        Response::Bye(reason) => {
            return Err(BackendError::Auth(format!("server BYE: {reason}")));
        }
    }

    // PUTSCRIPT name {len+}\r\n<bytes>\r\n
    // The {len+} synchronizing-literal tells the server to read
    // exactly len bytes after the CRLF; we then send another CRLF.
    let cmd = format!(
        "PUTSCRIPT \"{}\" {{{}+}}\r\n",
        escape_quoted(script_name),
        script.len()
    );
    tls.get_mut()
        .write_all(cmd.as_bytes())
        .await
        .map_err(|e| BackendError::Transport(format!("PUTSCRIPT write: {e}")))?;
    tls.get_mut()
        .write_all(script.as_bytes())
        .await
        .map_err(|e| BackendError::Transport(format!("PUTSCRIPT body: {e}")))?;
    tls.get_mut()
        .write_all(b"\r\n")
        .await
        .map_err(|e| BackendError::Transport(format!("PUTSCRIPT trailer: {e}")))?;
    match read_response(&mut tls).await? {
        Response::Ok => {}
        Response::No(reason) => {
            return Err(BackendError::Protocol(format!("PUTSCRIPT: {reason}")));
        }
        Response::Bye(reason) => {
            return Err(BackendError::Protocol(format!("server BYE: {reason}")));
        }
    }

    write_command(
        tls.get_mut(),
        &format!("SETACTIVE \"{}\"", escape_quoted(script_name)),
    )
    .await?;
    match read_response(&mut tls).await? {
        Response::Ok => {}
        Response::No(reason) => {
            return Err(BackendError::Protocol(format!("SETACTIVE: {reason}")));
        }
        Response::Bye(reason) => {
            return Err(BackendError::Protocol(format!("server BYE: {reason}")));
        }
    }

    // Best-effort logout; we already have what we wanted.
    let _ = tls.get_mut().write_all(b"LOGOUT\r\n").await;
    Ok(())
}

/// Build the rustls config used for the STARTTLS upgrade. Same
/// posture as the IMAP backend: ring provider + Mozilla roots, no
/// client auth, no insecure fallbacks.
fn tls_config() -> ClientConfig {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

#[derive(Debug)]
enum Response {
    Ok,
    No(String),
    Bye(String),
}

/// Read the multi-line greeting that `ManageSieve` sends right after
/// connect and after STARTTLS. The greeting is a sequence of
/// capability lines followed by `OK ...` (or `BYE ...` if the
/// server is shutting us down).
///
/// We don't currently parse capability content; a misbehaving
/// server will surface as a later Protocol error.
async fn read_until_ok<S>(stream: &mut BufReader<S>) -> Result<(), BackendError>
where
    S: tokio::io::AsyncRead + Unpin,
{
    // `read_response` already drains capability lines internally; we
    // just classify the terminator.
    match read_response(stream).await? {
        Response::Ok => Ok(()),
        Response::No(reason) => Err(BackendError::Protocol(format!("server NO: {reason}"))),
        Response::Bye(reason) => Err(BackendError::Transport(format!("server BYE: {reason}"))),
    }
}

/// Read exactly one `ManageSieve` response (capability lines + final
/// terminator). Returns the terminator type. Capability lines are
/// drained but not surfaced — the v0 client doesn't act on them.
///
/// The terminator is one of `OK`, `NO`, or `BYE`, optionally
/// followed by space + human-readable detail. We capture the detail
/// for error variants so the caller's error message names what the
/// server actually said.
async fn read_response<S>(stream: &mut BufReader<S>) -> Result<Response, BackendError>
where
    S: tokio::io::AsyncRead + Unpin,
{
    loop {
        let mut line = String::new();
        let n = stream
            .read_line(&mut line)
            .await
            .map_err(|e| BackendError::Transport(format!("read: {e}")))?;
        if n == 0 {
            return Err(BackendError::Transport("eof".into()));
        }
        let trimmed = line.trim_end_matches(['\r', '\n']).to_string();

        // Literal: `{N}` at end of line means the server is sending
        // a sized blob immediately after. Drain N bytes + the
        // following CRLF so we don't desync.
        if let Some(literal_len) = parse_literal_marker(&trimmed) {
            let mut buf = vec![0u8; literal_len];
            stream
                .read_exact(&mut buf)
                .await
                .map_err(|e| BackendError::Transport(format!("literal body: {e}")))?;
            // Discard trailing CRLF that follows the literal.
            let mut crlf = String::new();
            stream
                .read_line(&mut crlf)
                .await
                .map_err(|e| BackendError::Transport(format!("literal trailer: {e}")))?;
            continue;
        }

        // Terminators are unquoted at column 0.
        if let Some(rest) = trimmed.strip_prefix("OK") {
            let _ = rest;
            return Ok(Response::Ok);
        }
        if let Some(rest) = trimmed.strip_prefix("NO") {
            return Ok(Response::No(rest.trim().to_string()));
        }
        if let Some(rest) = trimmed.strip_prefix("BYE") {
            return Ok(Response::Bye(rest.trim().to_string()));
        }
        // Otherwise it's a capability or status line; ignore + loop.
    }
}

/// Detect a trailing `{N}` or `{N+}` literal marker on a line.
/// Returns the declared body length if present. The `+` form is
/// the non-synchronizing literal — the server doesn't wait for our
/// "OK send the bytes" before we transmit them.
fn parse_literal_marker(line: &str) -> Option<usize> {
    let close = line.rfind('}')?;
    let open = line[..close].rfind('{')?;
    let inner = line[open + 1..close].trim_end_matches('+');
    inner.parse::<usize>().ok()
}

async fn write_command<S>(stream: &mut S, cmd: &str) -> Result<(), BackendError>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    stream
        .write_all(cmd.as_bytes())
        .await
        .map_err(|e| BackendError::Transport(format!("write {cmd}: {e}")))?;
    stream
        .write_all(b"\r\n")
        .await
        .map_err(|e| BackendError::Transport(format!("write {cmd} trailer: {e}")))?;
    Ok(())
}

/// Escape `"` in a quoted-string argument. `ManageSieve` quoted strings
/// follow the IMAP convention: backslash escapes the quote and
/// backslash itself.
fn escape_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out
}

/// Minimal base64 encoder. We only need to encode auth blobs (a
/// few hundred bytes max) so a hand-rolled implementation is fine
/// and saves a dependency.
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        out.push(CHARS[(b2 & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let b0 = input[i];
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[((b0 & 0x03) << 4) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = input[i];
        let b1 = input[i + 1];
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(CHARS[((b1 & 0x0f) << 2) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_literal_marker_picks_trailing_braces() {
        assert_eq!(parse_literal_marker("PUTSCRIPT \"x\" {42}"), Some(42));
        assert_eq!(parse_literal_marker("PUTSCRIPT \"x\" {42+}"), Some(42));
        assert_eq!(parse_literal_marker("OK \"done\""), None);
        assert_eq!(parse_literal_marker(""), None);
    }

    #[test]
    fn escape_quoted_handles_quote_and_backslash() {
        assert_eq!(escape_quoted("simple"), "simple");
        assert_eq!(escape_quoted("with \"quote\""), "with \\\"quote\\\"");
        assert_eq!(escape_quoted("with\\backslash"), "with\\\\backslash");
    }

    #[test]
    fn base64_encodes_known_vectors() {
        // RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_encodes_sasl_plain_blob() {
        // \0user@host\0pass
        let blob = b"\0user@host\0pass";
        let encoded = base64_encode(blob);
        // Sanity: round-trip via std doesn't exist without a base64
        // decoder. Pin the expected output instead.
        assert_eq!(encoded, "AHVzZXJAaG9zdABwYXNz");
    }
}
