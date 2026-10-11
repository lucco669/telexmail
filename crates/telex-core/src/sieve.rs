//! Sieve on the server (RFC 5228), managed over ManageSieve (RFC 5804): how an away reply
//! (RFC 5230's `vacation`) gets onto the server, so it answers even while Telex is closed.
//!
//! Servers run one script at a time, and it may hold the user's own filters (from webmail or
//! another program). So Telex writes its part into the active script between two marked comment
//! lines, after the script's `require` lines, and later takes exactly that part out again; a
//! script Telex made itself is removed when it is left empty. The server checks every script it
//! is given, and keeps the old one when it refuses the new.
//!
//! The connection is the IMAP server's name on port 4190, encrypted with STARTTLS before the
//! login; a server that doesn't offer STARTTLS is never sent the password.

use std::fmt;
use std::time::Duration;

use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::tr;

pub const PORT: u16 = 4190;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest line, and the largest string (a script), Telex takes from a server: far more
/// than any real one, little enough that a hostile server can't fill the memory.
const MAX_LINE: u64 = 64 * 1024;
const MAX_LITERAL: usize = 1024 * 1024;
/// The name of the script Telex makes when the server has none running.
pub const OWN_SCRIPT: &str = "telex";
const BEGIN: &str = "# Telex: away reply, begin. Telex writes and removes this part.";
const END: &str = "# Telex: away reply, end.";

/// Why Telex couldn't put a reply on the server. The UI words the codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SieveError {
    /// Nothing answers ManageSieve on port 4190, or not in time.
    NoServer,
    /// It answers, but won't encrypt the connection.
    NoTls,
    /// It doesn't do Sieve's `vacation`, or not with dates.
    NoVacation,
    NoDates,
    /// It refused the login.
    Login(String),
    /// It refused a command or a script, in its words.
    Refused(String),
}

impl SieveError {
    /// A code for the UI: why the reply runs in Telex instead.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoServer => "no_server",
            Self::NoTls => "no_tls",
            Self::NoVacation => "no_vacation",
            Self::NoDates => "no_dates",
            Self::Login(_) => "login",
            Self::Refused(_) => "refused",
        }
    }
}

impl fmt::Display for SieveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::NoServer => tr!(
                "the server doesn't take filters from mail programs (ManageSieve)",
                "serwer nie przyjmuje filtrów od programów pocztowych (ManageSieve)"
            ),
            Self::NoTls => tr!(
                "the server's filter service doesn't encrypt the connection",
                "usługa filtrów serwera nie szyfruje połączenia"
            ),
            Self::NoVacation => tr!(
                "the server's filters can't send automatic replies",
                "filtry serwera nie umieją wysyłać automatycznych odpowiedzi"
            ),
            Self::NoDates => tr!(
                "the server's filters can't keep to dates",
                "filtry serwera nie umieją trzymać się dat"
            ),
            Self::Login(why) => tr!(
                "the server's filter service refused the login ({why})",
                "usługa filtrów serwera odrzuciła logowanie ({why})"
            ),
            Self::Refused(why) => tr!("the server refused it: {why}", "serwer odmówił: {why}"),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for SieveError {}

/// What the server says it can do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub sieve: Vec<String>,
    pub sasl: Vec<String>,
    pub starttls: bool,
}

impl Capabilities {
    pub fn has(&self, extension: &str) -> bool {
        self.sieve.iter().any(|e| e.eq_ignore_ascii_case(extension))
    }
}

/// One answer: the lines before it (strings and words) and its status.
#[derive(Debug, Default)]
struct Response {
    ok: bool,
    lines: Vec<Vec<String>>,
    text: String,
}

/// A ManageSieve connection over any stream: TLS in the app, plain TCP in the tests.
pub struct Session<S> {
    stream: BufReader<S>,
    pub caps: Capabilities,
}

fn lost() -> SieveError {
    SieveError::Refused(tr!(
        "the connection was closed",
        "połączenie zostało zamknięte"
    ))
}

/// A string as a ManageSieve literal the server takes without waiting: `{12+}` and the text.
fn literal(text: &str) -> String {
    format!("{{{}+}}\r\n{text}", text.len())
}

/// A string in quotes, for names and short values.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

impl<S: AsyncRead + AsyncWrite + Unpin> Session<S> {
    /// Read the greeting: the server's capabilities.
    pub async fn greet(stream: S) -> Result<Self, SieveError> {
        let mut session = Self {
            stream: BufReader::new(stream),
            caps: Capabilities::default(),
        };
        session.read_caps().await?;
        Ok(session)
    }

    async fn read_caps(&mut self) -> Result<(), SieveError> {
        let r = self.response().await?;
        if !r.ok {
            return Err(SieveError::Refused(r.text));
        }
        let mut caps = Capabilities::default();
        for line in r.lines {
            let value = |i: usize| line.get(i).cloned().unwrap_or_default();
            match value(0).to_ascii_uppercase().as_str() {
                "SIEVE" => caps.sieve = value(1).split_whitespace().map(str::to_owned).collect(),
                "SASL" => caps.sasl = value(1).split_whitespace().map(str::to_owned).collect(),
                "STARTTLS" => caps.starttls = true,
                _ => {}
            }
        }
        self.caps = caps;
        Ok(())
    }

    /// Read one line, and the literals it ends with, into words and strings.
    async fn line(&mut self) -> Result<Vec<String>, SieveError> {
        let mut words = Vec::new();
        loop {
            let mut raw = Vec::new();
            let mut limited = (&mut self.stream).take(MAX_LINE);
            let n = timeout(COMMAND_TIMEOUT, limited.read_until(b'\n', &mut raw))
                .await
                .map_err(|_| lost())?
                .map_err(|_| lost())?;
            if n == 0 || (n as u64 == MAX_LINE && !raw.ends_with(b"\n")) {
                return Err(lost());
            }
            let text = String::from_utf8_lossy(&raw);
            let text = text.trim_end_matches(['\r', '\n']);
            let mut rest = text;
            let mut literal_len = None;
            while let Some(c) = rest.chars().next() {
                if c == ' ' {
                    rest = &rest[1..];
                } else if c == '"' {
                    let mut out = String::new();
                    let mut chars = rest[1..].char_indices();
                    let mut end = rest.len();
                    while let Some((i, c)) = chars.next() {
                        match c {
                            '\\' => out.extend(chars.next().map(|(_, c)| c)),
                            '"' => {
                                end = i + 2;
                                break;
                            }
                            c => out.push(c),
                        }
                    }
                    words.push(out);
                    rest = &rest[end.min(rest.len())..];
                } else if c == '{' && rest.ends_with('}') {
                    let n = rest[1..rest.len() - 1].trim_end_matches('+');
                    literal_len = n.parse::<usize>().ok();
                    rest = "";
                } else if c == '(' {
                    let end = rest.find(')').map_or(rest.len(), |i| i + 1);
                    words.push(rest[..end].to_owned());
                    rest = &rest[end..];
                } else {
                    let end = rest.find(' ').unwrap_or(rest.len());
                    words.push(rest[..end].to_owned());
                    rest = &rest[end..];
                }
            }
            match literal_len {
                Some(n) if n > MAX_LITERAL => return Err(lost()),
                Some(n) => {
                    let mut data = vec![0; n];
                    timeout(COMMAND_TIMEOUT, self.stream.read_exact(&mut data))
                        .await
                        .map_err(|_| lost())?
                        .map_err(|_| lost())?;
                    words.push(String::from_utf8_lossy(&data).into_owned());
                }
                None => return Ok(words),
            }
        }
    }

    /// Lines until OK, NO or BYE.
    async fn response(&mut self) -> Result<Response, SieveError> {
        let mut r = Response::default();
        loop {
            let words = self.line().await?;
            let first = words.first().map(|w| w.to_ascii_uppercase());
            match first.as_deref() {
                Some("OK") | Some("NO") | Some("BYE") => {
                    r.ok = first.as_deref() == Some("OK");
                    r.text = words
                        .iter()
                        .skip(1)
                        .filter(|w| !w.starts_with('('))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ");
                    return Ok(r);
                }
                _ => r.lines.push(words),
            }
        }
    }

    async fn command(&mut self, line: &str) -> Result<Response, SieveError> {
        let stream = self.stream.get_mut();
        timeout(COMMAND_TIMEOUT, async {
            stream.write_all(line.as_bytes()).await?;
            stream.write_all(b"\r\n").await?;
            stream.flush().await
        })
        .await
        .map_err(|_| lost())?
        .map_err(|_| lost())?;
        self.response().await
    }

    async fn ok(&mut self, line: &str) -> Result<Response, SieveError> {
        let r = self.command(line).await?;
        if r.ok {
            Ok(r)
        } else {
            Err(SieveError::Refused(r.text))
        }
    }

    /// Log in with SASL PLAIN.
    pub async fn login(&mut self, user: &str, password: &str) -> Result<(), SieveError> {
        if !self
            .caps
            .sasl
            .iter()
            .any(|m| m.eq_ignore_ascii_case("PLAIN"))
        {
            return Err(SieveError::Login("PLAIN".into()));
        }
        let token =
            base64::engine::general_purpose::STANDARD.encode(format!("\0{user}\0{password}"));
        let r = self
            .command(&format!("AUTHENTICATE \"PLAIN\" {}", quoted(&token)))
            .await?;
        if r.ok {
            Ok(())
        } else {
            Err(SieveError::Login(r.text))
        }
    }

    /// The scripts, and which one runs.
    pub async fn scripts(&mut self) -> Result<(Vec<String>, Option<String>), SieveError> {
        let r = self.ok("LISTSCRIPTS").await?;
        let mut names = Vec::new();
        let mut active = None;
        for line in r.lines {
            let Some(name) = line.first() else { continue };
            if line
                .get(1)
                .is_some_and(|w| w.eq_ignore_ascii_case("ACTIVE"))
            {
                active = Some(name.clone());
            }
            names.push(name.clone());
        }
        Ok((names, active))
    }

    pub async fn get(&mut self, name: &str) -> Result<String, SieveError> {
        let r = self.ok(&format!("GETSCRIPT {}", quoted(name))).await?;
        Ok(r.lines.into_iter().flatten().next().unwrap_or_default())
    }

    pub async fn put(&mut self, name: &str, script: &str) -> Result<(), SieveError> {
        self.ok(&format!("PUTSCRIPT {} {}", quoted(name), literal(script)))
            .await
            .map(|_| ())
    }

    /// Make `name` the script that runs; "" stops them all.
    pub async fn activate(&mut self, name: &str) -> Result<(), SieveError> {
        self.ok(&format!("SETACTIVE {}", quoted(name)))
            .await
            .map(|_| ())
    }

    pub async fn delete(&mut self, name: &str) -> Result<(), SieveError> {
        self.ok(&format!("DELETESCRIPT {}", quoted(name)))
            .await
            .map(|_| ())
    }

    pub async fn logout(&mut self) {
        let _ = self.command("LOGOUT").await;
    }

    /// Put `block` into the running script (or a script of Telex's own when none runs).
    /// Returns the script's name, and whether Telex made it.
    pub async fn install(&mut self, block: &str) -> Result<(String, bool), SieveError> {
        let (names, active) = self.scripts().await?;
        match active {
            Some(name) => {
                let script = self.get(&name).await?;
                self.put(&name, &with_block(&script, block)).await?;
                let own = name == OWN_SCRIPT;
                Ok((name, own))
            }
            None => {
                // A script of Telex's own that isn't running: start again from it.
                let base = if names.iter().any(|n| n == OWN_SCRIPT) {
                    self.get(OWN_SCRIPT).await?
                } else {
                    String::new()
                };
                self.put(OWN_SCRIPT, &with_block(&base, block)).await?;
                self.activate(OWN_SCRIPT).await?;
                Ok((OWN_SCRIPT.to_owned(), true))
            }
        }
    }

    /// Take Telex's part out of `name`; a script Telex made that is left empty goes.
    pub async fn uninstall(&mut self, name: &str, created: bool) -> Result<(), SieveError> {
        let (names, active) = self.scripts().await?;
        if !names.iter().any(|n| n == name) {
            return Ok(()); // removed meanwhile
        }
        let script = self.get(name).await?;
        let left = without_block(&script);
        if created && left.trim().is_empty() {
            if active.as_deref() == Some(name) {
                self.activate("").await?;
            }
            return self.delete(name).await;
        }
        if left != script {
            self.put(name, &left).await?;
        }
        Ok(())
    }
}

/// Connect to `host`'s ManageSieve, encrypt with STARTTLS and log in.
pub async fn connect(
    host: &str,
    user: &str,
    password: &str,
) -> Result<Session<tokio_rustls::client::TlsStream<TcpStream>>, SieveError> {
    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect((host, PORT)))
        .await
        .map_err(|_| SieveError::NoServer)?
        .map_err(|_| SieveError::NoServer)?;
    let _ = tcp.set_nodelay(true);
    let mut plain = timeout(CONNECT_TIMEOUT, Session::greet(tcp))
        .await
        .map_err(|_| SieveError::NoServer)?
        .map_err(|_| SieveError::NoServer)?;
    if !plain.caps.starttls {
        return Err(SieveError::NoTls);
    }
    plain.ok("STARTTLS").await.map_err(|_| SieveError::NoTls)?;
    let name = ServerName::try_from(host.to_owned()).map_err(|_| SieveError::NoServer)?;
    let tcp = plain.stream.into_inner();
    let tls = timeout(
        CONNECT_TIMEOUT,
        crate::imap::tls_connector().connect(name, tcp),
    )
    .await
    .map_err(|_| SieveError::NoTls)?
    .map_err(|_| SieveError::NoTls)?;
    // After STARTTLS the server says what it can do again, now over TLS.
    let mut session = Session::greet(tls).await?;
    session.login(user, password).await?;
    Ok(session)
}

// ---- The script ----

/// A Sieve string: in quotes, or for longer text a `text:` block ending in a line with a
/// single dot (a line of the text that starts with a dot gets another one).
fn sieve_string(text: &str) -> String {
    if !text.contains('\n') && text.chars().count() < 200 {
        return quoted(text);
    }
    let mut out = String::from("text:\r\n");
    for line in text.replace("\r\n", "\n").lines() {
        if line.starts_with('.') {
            out.push('.');
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str(".\r\n");
    out
}

/// What Telex puts into the script: the `vacation` command, inside a condition on the dates
/// when there are any.
pub struct Vacation<'a> {
    pub subject: &'a str,
    pub body: &'a str,
    pub days: u16,
    /// The account's addresses: mail to any of them is answered.
    pub addresses: &'a [String],
    pub starts: Option<chrono::NaiveDate>,
    pub ends: Option<chrono::NaiveDate>,
}

impl Vacation<'_> {
    pub fn block(&self) -> String {
        let dated = self.starts.is_some() || self.ends.is_some();
        let mut require = vec!["\"vacation\""];
        if dated {
            require.extend(["\"date\"", "\"relational\""]);
        }
        let mut action = format!("vacation :days {}", self.days.clamp(1, 365));
        if !self.subject.trim().is_empty() {
            action.push_str(&format!(" :subject {}", quoted(self.subject.trim())));
        }
        if !self.addresses.is_empty() {
            let list: Vec<String> = self.addresses.iter().map(|a| quoted(a)).collect();
            action.push_str(&format!(" :addresses [{}]", list.join(", ")));
        }
        action.push(' ');
        action.push_str(&sieve_string(self.body.trim_end()));
        action.push_str(";\r\n");
        let mut block = format!("{BEGIN}\r\nrequire [{}];\r\n", require.join(", "));
        if dated {
            let mut tests = Vec::new();
            if let Some(d) = self.starts {
                tests.push(format!("currentdate :value \"ge\" \"date\" \"{d}\""));
            }
            if let Some(d) = self.ends {
                tests.push(format!("currentdate :value \"le\" \"date\" \"{d}\""));
            }
            block.push_str(&format!(
                "if allof ({}) {{\r\n{action}}}\r\n",
                tests.join(", ")
            ));
        } else {
            block.push_str(&action);
        }
        block.push_str(END);
        block.push_str("\r\n");
        block
    }
}

/// Where a script's leading part ends: blank lines, comments and `require` commands.
fn after_requires(script: &str) -> usize {
    let bytes = script.as_bytes();
    let mut at = 0;
    let mut end = 0;
    while at < bytes.len() {
        let rest = &script[at..];
        if rest.starts_with(char::is_whitespace) {
            at += rest.chars().next().map_or(1, char::len_utf8);
        } else if rest.starts_with('#') {
            at += rest.find('\n').map_or(rest.len(), |i| i + 1);
            end = at;
        } else if rest.starts_with("/*") {
            at += rest.find("*/").map_or(rest.len(), |i| i + 2);
            end = at;
        } else if rest
            .as_bytes()
            .get(..7)
            .is_some_and(|w| w.eq_ignore_ascii_case(b"require"))
        {
            // Up to its ";", past strings that may hold one.
            let mut quoted = false;
            let mut escaped = false;
            let mut stop = rest.len();
            for (i, c) in rest.char_indices() {
                match c {
                    _ if escaped => escaped = false,
                    '\\' if quoted => escaped = true,
                    '"' => quoted = !quoted,
                    ';' if !quoted => {
                        stop = i + 1;
                        break;
                    }
                    _ => {}
                }
            }
            at += stop;
            if let Some(nl) = script[at..].find('\n') {
                if script[at..at + nl].trim().is_empty() {
                    at += nl + 1;
                }
            }
            end = at;
        } else {
            break;
        }
    }
    end
}

/// The script without Telex's part.
pub fn without_block(script: &str) -> String {
    let Some(start) = script.find(BEGIN) else {
        return script.to_owned();
    };
    let Some(stop) = script[start..].find(END).map(|i| start + i + END.len()) else {
        return script.to_owned();
    };
    let after = script[stop..]
        .strip_prefix("\r\n")
        .or_else(|| script[stop..].strip_prefix('\n'))
        .unwrap_or(&script[stop..]);
    format!("{}{after}", &script[..start])
}

/// The script with Telex's part (a new one in place of any old one) after its `require` lines,
/// before its first command, so the reply goes out before any of its rules can stop.
pub fn with_block(script: &str, block: &str) -> String {
    let script = without_block(script);
    let at = after_requires(&script);
    let mut head = script[..at].to_owned();
    if !head.is_empty() && !head.ends_with('\n') {
        head.push_str("\r\n");
    }
    format!("{head}{block}{}", &script[at..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
    use tokio::net::TcpListener;

    fn vacation(starts: Option<NaiveDate>) -> String {
        let addresses = [
            "biuro@twojastudio.pl".to_owned(),
            "anna@twojastudio.pl".to_owned(),
        ];
        Vacation {
            subject: "Urlop \"do\" 20.10",
            body: "Dzień dobry,\n.jestem na urlopie.\n\nAnna",
            days: 4,
            addresses: &addresses,
            starts,
            ends: NaiveDate::from_ymd_opt(2026, 10, 20),
        }
        .block()
    }

    #[test]
    fn the_reply_goes_into_a_script_and_comes_out_whole() {
        let block = vacation(NaiveDate::from_ymd_opt(2026, 10, 12));
        assert!(block.contains("require [\"vacation\", \"date\", \"relational\"];"));
        assert!(block.contains(
            "if allof (currentdate :value \"ge\" \"date\" \"2026-10-12\", currentdate :value \"le\" \"date\" \"2026-10-20\") {"
        ));
        assert!(block.contains(":subject \"Urlop \\\"do\\\" 20.10\""));
        assert!(block.contains(":addresses [\"biuro@twojastudio.pl\", \"anna@twojastudio.pl\"]"));
        assert!(
            block.contains("text:\r\nDzień dobry,\r\n..jestem na urlopie.\r\n\r\nAnna\r\n.\r\n;")
        );

        // The user's own filters stay; Telex's part goes after their requires.
        let theirs = "# Roundcube\r\nrequire [\"fileinto\"];\r\nrequire \"imap4flags\";\r\n\r\nif header :contains \"subject\" \"faktura\" {\r\n  fileinto \"Faktury\";\r\n  stop;\r\n}\r\n";
        let edited = with_block(theirs, &block);
        let at_block = edited.find(BEGIN).unwrap();
        assert!(edited.find("require \"imap4flags\";").unwrap() < at_block);
        assert!(
            at_block < edited.find("if header").unwrap(),
            "before their first rule"
        );
        assert_eq!(without_block(&edited), theirs, "taken out exactly");
        // Saving again replaces Telex's part instead of adding a second one.
        let again = with_block(&edited, &vacation(None));
        assert_eq!(again.matches(BEGIN).count(), 1);
        assert!(!again.contains("2026-10-12"));
        // A script with nothing but Telex's part is empty without it.
        assert!(without_block(&with_block("", &block)).trim().is_empty());
        // Text that isn't ASCII right where a command would start.
        assert!(with_block("keep;#żółw", &block).contains(BEGIN));
        assert!(with_block("żółw", &block).starts_with(BEGIN));
        // A semicolon inside a require's string doesn't end it.
        let odd = "require [\"a;b\"];\r\nkeep;\r\n";
        assert!(with_block(odd, &block).starts_with("require [\"a;b\"];\r\n# Telex"));
    }

    /// The fake server's scripts: name, text, running.
    type Scripts = std::sync::Arc<std::sync::Mutex<Vec<(String, String, bool)>>>;

    /// A ManageSieve server that knows a few commands and keeps scripts in memory.
    async fn fake_server(listener: TcpListener, scripts: Scripts) {
        let (socket, _) = listener.accept().await.unwrap();
        let (read, mut write) = socket.into_split();
        let mut read = TokioBufReader::new(read);
        let caps = "\"IMPLEMENTATION\" \"Fake\"\r\n\"SIEVE\" \"fileinto vacation date relational\"\r\n\"SASL\" \"PLAIN\"\r\n\"VERSION\" \"1.0\"\r\nOK\r\n";
        write.write_all(caps.as_bytes()).await.unwrap();
        loop {
            let mut line = String::new();
            if read.read_line(&mut line).await.unwrap() == 0 {
                return;
            }
            let line = line.trim_end().to_owned();
            let arg = |i: usize| line.split('"').nth(i).unwrap_or("").to_owned();
            let reply = if line.starts_with("AUTHENTICATE") {
                let token = base64::engine::general_purpose::STANDARD
                    .decode(arg(3))
                    .unwrap();
                if token == b"\0anna\0tajne" {
                    "OK\r\n".into()
                } else {
                    "NO \"Bad password\"\r\n".into()
                }
            } else if line.starts_with("LISTSCRIPTS") {
                let s = scripts.lock().unwrap();
                let mut out: String = s
                    .iter()
                    .map(|(n, _, a)| format!("\"{n}\"{}\r\n", if *a { " ACTIVE" } else { "" }))
                    .collect();
                out.push_str("OK\r\n");
                out
            } else if line.starts_with("GETSCRIPT") {
                let s = scripts.lock().unwrap();
                let body = s
                    .iter()
                    .find(|(n, _, _)| *n == arg(1))
                    .map(|(_, b, _)| b.clone())
                    .unwrap_or_default();
                format!("{{{}}}\r\n{body}\r\nOK\r\n", body.len())
            } else if line.starts_with("PUTSCRIPT") {
                let len: usize = line
                    .rsplit('{')
                    .next()
                    .unwrap()
                    .trim_end_matches("+}")
                    .parse()
                    .unwrap();
                let mut body = vec![0; len];
                read.read_exact(&mut body).await.unwrap();
                let mut rest = String::new();
                read.read_line(&mut rest).await.unwrap();
                let body = String::from_utf8(body).unwrap();
                if body.contains("broken") {
                    "NO \"line 1: syntax error\"\r\n".into()
                } else {
                    let mut s = scripts.lock().unwrap();
                    match s.iter_mut().find(|(n, _, _)| *n == arg(1)) {
                        Some(found) => found.1 = body,
                        None => s.push((arg(1), body, false)),
                    }
                    "OK\r\n".into()
                }
            } else if line.starts_with("SETACTIVE") {
                let mut s = scripts.lock().unwrap();
                for (n, _, a) in s.iter_mut() {
                    *a = *n == arg(1);
                }
                "OK\r\n".into()
            } else if line.starts_with("DELETESCRIPT") {
                scripts.lock().unwrap().retain(|(n, _, _)| *n != arg(1));
                "OK\r\n".into()
            } else if line.starts_with("LOGOUT") {
                write.write_all(b"OK \"Bye\"\r\n").await.unwrap();
                return;
            } else {
                "NO \"Unknown command\"\r\n".into()
            };
            write.write_all(reply.as_bytes()).await.unwrap();
        }
    }

    async fn session(scripts: &Scripts) -> Session<TcpStream> {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(fake_server(listener, scripts.clone()));
        let tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        Session::greet(tcp).await.unwrap()
    }

    #[tokio::test]
    async fn a_server_announcing_a_huge_string_is_refused() {
        let (client, mut server) = tokio::io::duplex(1024);
        server
            .write_all(b"\"IMPLEMENTATION\" {99999999999}\r\n")
            .await
            .unwrap();
        assert!(Session::greet(client).await.is_err());
        let (client, mut server) = tokio::io::duplex(256 * 1024);
        let endless = vec![b'x'; 100 * 1024];
        server.write_all(&endless).await.unwrap();
        assert!(Session::greet(client).await.is_err(), "a line without end");
    }

    #[tokio::test]
    async fn a_reply_is_put_on_the_server_and_taken_off_again() {
        use std::sync::{Arc, Mutex};
        let theirs = "require \"fileinto\";\r\nif header :contains \"subject\" \"faktura\" { fileinto \"Faktury\"; }\r\n";
        let scripts = Arc::new(Mutex::new(vec![(
            "roundcube".to_owned(),
            theirs.to_owned(),
            true,
        )]));
        let mut s = session(&scripts).await;
        assert!(s.caps.has("vacation") && s.caps.has("date"));
        assert!(matches!(
            s.login("anna", "zle").await,
            Err(SieveError::Login(_))
        ));
        s.login("anna", "tajne").await.unwrap();
        let (name, created) = s.install(&vacation(None)).await.unwrap();
        assert_eq!(
            (name.as_str(), created),
            ("roundcube", false),
            "into the running script"
        );
        assert!(scripts.lock().unwrap()[0].1.contains("vacation :days 4"));
        // A script the server refuses changes nothing.
        assert!(matches!(
            s.install("broken").await,
            Err(SieveError::Refused(_))
        ));
        s.uninstall("roundcube", false).await.unwrap();
        assert_eq!(
            scripts.lock().unwrap()[0].1,
            theirs,
            "their filters as they were"
        );
        s.logout().await;

        // No script running: Telex makes its own, then removes it.
        let scripts = Arc::new(Mutex::new(Vec::new()));
        let mut s = session(&scripts).await;
        s.login("anna", "tajne").await.unwrap();
        assert_eq!(
            s.install(&vacation(None)).await.unwrap(),
            (OWN_SCRIPT.to_owned(), true)
        );
        assert!(scripts.lock().unwrap()[0].2, "and runs it");
        s.uninstall(OWN_SCRIPT, true).await.unwrap();
        assert!(scripts.lock().unwrap().is_empty());
    }
}
