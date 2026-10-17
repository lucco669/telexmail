//! Finding an account's server settings from its email address, for the add-account wizard.
//!
//! Tried in order; the first that knows wins:
//! 1. Telex's own list of common providers (no network). It only saves time and adds tips
//!    such as "use an app password"; everything below works for any provider anywhere.
//! 2. The provider's own setup file in Thunderbird's autoconfig format, from
//!    `autoconfig.<domain>` or `<domain>/.well-known/`. Hosting panels such as cPanel serve it.
//! 3. SRV records in the domain's DNS zone (RFC 6186 and 8314): `_imaps._tcp` (else
//!    `_pop3s._tcp`), `_submissions._tcp` and `_submission._tcp`.
//! 4. The domain's MX records, when its mail is handled by a provider on the list (Google
//!    Workspace, Microsoft 365, Zoho, OVH, ...).
//! 5. The usual server names (`imap.`, `mail.`, `smtp.`, the MX host, the domain itself), each
//!    checked with a TLS connection, a valid certificate and a mail server's greeting.
//!
//! Only DNS and the domain's own servers are asked, never a third-party lookup service. The
//! wizard then logs in with what was found, which is the real test.
//!
//! Tips and reasons are codes, not sentences: the UI words them, so they translate with it.

use std::time::Duration;

use futures_util::future::{join, join3, join4, join_all};
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::model::{AccountSetup, Pop3Keep, SmtpSecurity};

/// How long one probe or one download may take.
const WAIT: Duration = Duration::from_secs(6);

/// Where the settings came from, so the wizard can say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Telex's provider list.
    List,
    /// The provider's autoconfig file.
    Autoconfig,
    /// SRV records in the domain's DNS zone.
    Srv,
    /// The domain's MX records point to a provider on the list.
    Mx,
    /// A server that answered under one of the usual names.
    Guess,
}

/// Something to do before Telex can log in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Note {
    /// The provider takes an app password made for mail programs, not the usual one.
    AppPassword,
    /// Access for mail programs (IMAP) is off until the user turns it on.
    TurnOnImap,
    TurnOnImapAndAppPassword,
}

/// Why Telex can't use an account yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unsupported {
    /// Outlook.com and Microsoft 365 take only Microsoft sign-in (OAuth), roadmap step 5.
    MicrosoftSignIn,
    /// Proton Mail needs its Bridge, which speaks IMAP over STARTTLS only.
    ProtonBridge,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Detected {
    /// Ready for `add_account`. The name is left empty: the user gives one.
    pub setup: AccountSetup,
    /// The provider's name when Telex knows it, e.g. "Onet Poczta".
    pub provider: Option<String>,
    pub source: Source,
    pub note: Option<Note>,
    /// The wizard stops and says why.
    pub unsupported: Option<Unsupported>,
    /// A page of the provider's that helps with the note, e.g. where app passwords are made.
    pub help: Option<String>,
    /// The servers came from DNS (SRV records, or a guess that tried the MX hosts) and aren't
    /// in the address's own domain: the wizard asks before it sends them the password, as
    /// RFC 6186 says, since a forged DNS answer could have named them.
    pub confirm: bool,
}

/// Which login name a provider expects.
#[derive(Clone, Copy)]
enum Login {
    Address,
    /// The part before the @.
    LocalPart,
}

struct Provider {
    name: &'static str,
    help: Option<&'static str>,
    domains: &'static [&'static str],
    /// Ends of MX host names that mean a custom domain's mail is handled here.
    mx: &'static [&'static str],
    /// Always port 993, SSL/TLS.
    imap: &'static str,
    smtp: (&'static str, u16, SmtpSecurity),
    login: Login,
    note: Option<Note>,
    unsupported: Option<Unsupported>,
}

const TLS: SmtpSecurity = SmtpSecurity::Tls;
const STARTTLS: SmtpSecurity = SmtpSecurity::StartTls;

/// Checked against Mozilla's provider database and with a TLS connection to each server.
const PROVIDERS: &[Provider] = &[
    Provider {
        name: "Gmail",
        help: Some("https://myaccount.google.com/apppasswords"),
        domains: &["gmail.com", "googlemail.com"],
        mx: &["google.com", "googlemail.com"],
        imap: "imap.gmail.com",
        smtp: ("smtp.gmail.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::AppPassword),
        unsupported: None,
    },
    Provider {
        name: "Outlook.com",
        help: None,
        domains: OUTLOOK_DOMAINS,
        mx: &[],
        imap: "outlook.office365.com",
        smtp: ("smtp-mail.outlook.com", 587, STARTTLS),
        login: Login::Address,
        note: None,
        unsupported: Some(Unsupported::MicrosoftSignIn),
    },
    Provider {
        name: "Microsoft 365",
        help: None,
        domains: &[],
        mx: &["protection.outlook.com"],
        imap: "outlook.office365.com",
        smtp: ("smtp.office365.com", 587, STARTTLS),
        login: Login::Address,
        note: None,
        unsupported: Some(Unsupported::MicrosoftSignIn),
    },
    Provider {
        name: "iCloud Mail",
        help: None,
        domains: &["icloud.com", "me.com", "mac.com"],
        mx: &["mail.icloud.com"],
        imap: "imap.mail.me.com",
        smtp: ("smtp.mail.me.com", 587, STARTTLS),
        login: Login::Address,
        note: Some(Note::AppPassword),
        unsupported: None,
    },
    Provider {
        name: "Yahoo Mail",
        help: None,
        domains: &[
            "yahoo.com",
            "yahoo.ca",
            "yahoo.de",
            "yahoo.it",
            "yahoo.fr",
            "yahoo.es",
            "yahoo.se",
            "yahoo.co.in",
            "yahoo.co.uk",
            "yahoo.co.nz",
            "yahoo.com.au",
            "yahoo.com.ar",
            "yahoo.com.br",
            "yahoo.com.mx",
            "ymail.com",
            "myyahoo.com",
            "rocketmail.com",
        ],
        mx: &[],
        imap: "imap.mail.yahoo.com",
        smtp: ("smtp.mail.yahoo.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::AppPassword),
        unsupported: None,
    },
    Provider {
        name: "AOL Mail",
        help: None,
        domains: &[
            "aol.com",
            "aim.com",
            "aol.de",
            "aol.it",
            "aol.fr",
            "aol.es",
            "aol.co.uk",
        ],
        mx: &[],
        imap: "imap.aol.com",
        smtp: ("smtp.aol.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::AppPassword),
        unsupported: None,
    },
    Provider {
        name: "WP Poczta",
        help: None,
        domains: &["wp.pl"],
        mx: &[],
        imap: "imap.wp.pl",
        smtp: ("smtp.wp.pl", 465, TLS),
        login: Login::LocalPart,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "o2 Poczta",
        help: None,
        domains: &["o2.pl", "go2.pl", "tlen.pl", "prokonto.pl"],
        mx: &[],
        imap: "poczta.o2.pl",
        smtp: ("poczta.o2.pl", 465, TLS),
        login: Login::LocalPart,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "Onet Poczta",
        help: None,
        domains: &[
            "onet.pl",
            "onet.eu",
            "poczta.onet.pl",
            "poczta.onet.eu",
            "op.pl",
            "vp.pl",
            "autograf.pl",
            "buziaczek.pl",
            "amorki.pl",
            "republika.pl",
            "adres.pl",
            "cyberia.pl",
            "onet.com.pl",
            "opoczta.pl",
            "pseudonim.pl",
            "spoko.pl",
            "vip.onet.pl",
        ],
        mx: &[],
        imap: "imap.poczta.onet.pl",
        smtp: ("smtp.poczta.onet.pl", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "Interia Poczta",
        help: None,
        domains: &[
            "interia.pl",
            "interia.eu",
            "interia.com",
            "poczta.fm",
            "vip.interia.pl",
            "intmail.pl",
            "adresik.net",
        ],
        mx: &[],
        imap: "poczta.interia.pl",
        smtp: ("poczta.interia.pl", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "GMX",
        help: None,
        domains: &[
            "gmx.net", "gmx.de", "gmx.at", "gmx.ch", "gmx.eu", "gmx.biz", "gmx.org", "gmx.info",
        ],
        mx: &[],
        imap: "imap.gmx.net",
        smtp: ("mail.gmx.net", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "GMX",
        help: None,
        domains: &[
            "gmx.com",
            "gmx.us",
            "gmx.co.uk",
            "gmx.es",
            "gmx.fr",
            "gmx.ca",
            "gmx.it",
            "gmx.pt",
            "gmx.se",
            "gmx.ie",
        ],
        mx: &[],
        imap: "imap.gmx.com",
        smtp: ("mail.gmx.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "WEB.DE",
        help: None,
        domains: &["web.de"],
        mx: &[],
        imap: "imap.web.de",
        smtp: ("smtp.web.de", 465, TLS),
        login: Login::LocalPart,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "mail.com",
        help: None,
        domains: &[
            "mail.com",
            "email.com",
            "post.com",
            "usa.com",
            "myself.com",
            "consultant.com",
            "europe.com",
            "dr.com",
            "engineer.com",
            "techie.com",
        ],
        mx: &[],
        imap: "imap.mail.com",
        smtp: ("smtp.mail.com", 465, TLS),
        login: Login::Address,
        note: None,
        unsupported: None,
    },
    Provider {
        name: "Zoho Mail",
        help: None,
        domains: &["zoho.com", "zohomail.com"],
        mx: &[],
        imap: "imap.zoho.com",
        smtp: ("smtp.zoho.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "Zoho Mail",
        help: None,
        domains: &["zoho.eu", "zohomail.eu"],
        mx: &[],
        imap: "imap.zoho.eu",
        smtp: ("smtp.zoho.eu", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    // Zoho for your own domain uses its "pro" servers.
    Provider {
        name: "Zoho Mail",
        help: None,
        domains: &[],
        mx: &["zoho.com"],
        imap: "imappro.zoho.com",
        smtp: ("smtppro.zoho.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "Zoho Mail",
        help: None,
        domains: &[],
        mx: &["zoho.eu"],
        imap: "imappro.zoho.eu",
        smtp: ("smtppro.zoho.eu", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImap),
        unsupported: None,
    },
    Provider {
        name: "Fastmail",
        help: None,
        domains: &["fastmail.com", "fastmail.fm"],
        mx: &["messagingengine.com"],
        imap: "imap.fastmail.com",
        smtp: ("smtp.fastmail.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::AppPassword),
        unsupported: None,
    },
    Provider {
        name: "Yandex Mail",
        help: None,
        domains: &[
            "yandex.ru",
            "yandex.com",
            "yandex.by",
            "yandex.kz",
            "yandex.ua",
            "ya.ru",
            "narod.ru",
        ],
        mx: &["yandex.net", "yandex.ru"],
        imap: "imap.yandex.com",
        smtp: ("smtp.yandex.com", 465, TLS),
        login: Login::Address,
        note: Some(Note::TurnOnImapAndAppPassword),
        unsupported: None,
    },
    Provider {
        name: "Mail.ru",
        help: None,
        domains: &["mail.ru", "inbox.ru", "list.ru", "bk.ru"],
        mx: &[],
        imap: "imap.mail.ru",
        smtp: ("smtp.mail.ru", 465, TLS),
        login: Login::Address,
        note: Some(Note::AppPassword),
        unsupported: None,
    },
    Provider {
        name: "OVHcloud",
        help: None,
        domains: &[],
        mx: &["mail.ovh.net"],
        imap: "ssl0.ovh.net",
        smtp: ("ssl0.ovh.net", 465, TLS),
        login: Login::Address,
        note: None,
        unsupported: None,
    },
    Provider {
        name: "Purelymail",
        help: None,
        domains: &[],
        // Its MX, mailserver.purelymail.com, answers IMAP too, but the names it gives
        // mail programs are these.
        mx: &["purelymail.com"],
        imap: "imap.purelymail.com",
        smtp: ("smtp.purelymail.com", 465, TLS),
        login: Login::Address,
        note: None,
        unsupported: None,
    },
    Provider {
        name: "Proton Mail",
        help: None,
        domains: &["proton.me", "protonmail.com", "protonmail.ch", "pm.me"],
        mx: &["protonmail.ch"],
        imap: "127.0.0.1",
        smtp: ("127.0.0.1", 1025, STARTTLS),
        login: Login::Address,
        note: None,
        unsupported: Some(Unsupported::ProtonBridge),
    },
];

/// From Mozilla's provider database.
const OUTLOOK_DOMAINS: &[&str] = &[
    "hotmail.com",
    "live.com",
    "msn.com",
    "outlook.com",
    "windowslive.com",
    "outlook.at",
    "outlook.be",
    "outlook.cl",
    "outlook.cz",
    "outlook.de",
    "outlook.dk",
    "outlook.es",
    "outlook.fr",
    "outlook.hu",
    "outlook.ie",
    "outlook.in",
    "outlook.it",
    "outlook.jp",
    "outlook.kr",
    "outlook.lv",
    "outlook.my",
    "outlook.ph",
    "outlook.pt",
    "outlook.sa",
    "outlook.sg",
    "outlook.sk",
    "outlook.co.id",
    "outlook.co.il",
    "outlook.co.th",
    "outlook.com.ar",
    "outlook.com.au",
    "outlook.com.br",
    "outlook.com.gr",
    "outlook.com.tr",
    "outlook.com.vn",
    "hotmail.be",
    "hotmail.ca",
    "hotmail.cl",
    "hotmail.cz",
    "hotmail.de",
    "hotmail.dk",
    "hotmail.es",
    "hotmail.fi",
    "hotmail.fr",
    "hotmail.gr",
    "hotmail.hu",
    "hotmail.it",
    "hotmail.lt",
    "hotmail.lv",
    "hotmail.my",
    "hotmail.nl",
    "hotmail.no",
    "hotmail.ph",
    "hotmail.rs",
    "hotmail.se",
    "hotmail.sg",
    "hotmail.sk",
    "hotmail.co.id",
    "hotmail.co.il",
    "hotmail.co.in",
    "hotmail.co.jp",
    "hotmail.co.kr",
    "hotmail.co.th",
    "hotmail.co.uk",
    "hotmail.co.za",
    "hotmail.com.ar",
    "hotmail.com.au",
    "hotmail.com.br",
    "hotmail.com.hk",
    "hotmail.com.tr",
    "hotmail.com.tw",
    "hotmail.com.vn",
    "live.at",
    "live.be",
    "live.ca",
    "live.cl",
    "live.cn",
    "live.de",
    "live.dk",
    "live.fi",
    "live.fr",
    "live.hk",
    "live.ie",
    "live.in",
    "live.it",
    "live.jp",
    "live.nl",
    "live.no",
    "live.ru",
    "live.se",
    "live.co.jp",
    "live.co.kr",
    "live.co.uk",
    "live.co.za",
    "live.com.ar",
    "live.com.au",
    "live.com.mx",
    "live.com.my",
    "live.com.ph",
    "live.com.pt",
    "live.com.sg",
    "livemail.tw",
];

/// The address split into its local part and lowercase domain, or `None` for something
/// that isn't one.
fn split(email: &str) -> Option<(&str, String)> {
    let email = email.trim();
    let (local, domain) = email.rsplit_once('@')?;
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    let valid = !local.is_empty()
        && domain.contains('.')
        && !email.contains(char::is_whitespace)
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    valid.then_some((local, domain))
}

fn setup(
    email: &str,
    username: String,
    imap: &str,
    smtp: (&str, u16, SmtpSecurity),
) -> AccountSetup {
    AccountSetup {
        name: String::new(),
        email: email.trim().to_owned(),
        host: imap.to_owned(),
        port: 993,
        username,
        imap_security: SmtpSecurity::Tls,
        smtp_host: smtp.0.to_owned(),
        smtp_port: smtp.1,
        smtp_security: smtp.2,
        pop3: None,
    }
}

impl Provider {
    fn detected(&self, email: &str, source: Source) -> Option<Detected> {
        let (local, _) = split(email)?;
        let username = match self.login {
            Login::Address => email.trim().to_owned(),
            Login::LocalPart => local.to_owned(),
        };
        Some(Detected {
            provider: Some(self.name.to_owned()),
            note: self.note,
            unsupported: self.unsupported,
            help: self.help.map(str::to_owned),
            ..plain(setup(email, username, self.imap, self.smtp), source)
        })
    }
}

/// Settings found without the provider list: no name, tips or reasons.
fn plain(setup: AccountSetup, source: Source) -> Detected {
    let confirm = matches!(source, Source::Srv | Source::Guess) && outside_domain(&setup);
    Detected {
        setup,
        provider: None,
        source,
        note: None,
        unsupported: None,
        help: None,
        confirm,
    }
}

/// A server that isn't the address's domain or a name under it.
fn outside_domain(setup: &AccountSetup) -> bool {
    let Some((_, domain)) = setup.email.rsplit_once('@') else {
        return true;
    };
    let domain = domain.trim().to_ascii_lowercase();
    let inside = |host: &str| {
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        host == domain || host.ends_with(&format!(".{domain}"))
    };
    let smtp = if setup.smtp_host.trim().is_empty() {
        &setup.host
    } else {
        &setup.smtp_host
    };
    !(inside(&setup.host) && inside(smtp))
}

/// The provider list's settings for an address, without going online.
pub fn known(email: &str) -> Option<Detected> {
    let (_, domain) = split(email)?;
    PROVIDERS
        .iter()
        .find(|p| p.domains.contains(&domain.as_str()))?
        .detected(email, Source::List)
}

/// The settings for a domain whose MX records point to a provider on the list (a custom
/// domain on Google Workspace, Microsoft 365 and the like). `mx_hosts` in preference order.
pub fn from_mx(email: &str, mx_hosts: &[String]) -> Option<Detected> {
    mx_hosts.iter().find_map(|host| {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        PROVIDERS
            .iter()
            .find(|p| {
                p.mx.iter()
                    .any(|end| host == *end || host.ends_with(&format!(".{end}")))
            })?
            .detected(email, Source::Mx)
    })
}

// ---- Autoconfig files ----

/// The two places a provider publishes its setup file.
fn autoconfig_urls(email: &str, domain: &str) -> [String; 2] {
    let address: String = email
        .trim()
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    [
        format!("https://autoconfig.{domain}/mail/config-v1.1.xml?emailaddress={address}"),
        format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"),
    ]
}

/// Download the provider's setup file and read it. Blocking; `None` when there is none.
pub fn fetch_autoconfig(email: &str) -> Option<Detected> {
    let (_, domain) = split(email)?;
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(WAIT))
        .https_only(true)
        .max_redirects(3)
        .user_agent("Telex")
        .build();
    let agent = ureq::Agent::new_with_config(config);
    autoconfig_urls(email, &domain).iter().find_map(|url| {
        let mut response = agent.get(url).call().ok()?;
        let xml = response
            .body_mut()
            .with_config()
            .limit(256 * 1024)
            .read_to_string()
            .ok()?;
        parse_autoconfig(&xml, email)
    })
}

/// Settings from a setup file in Thunderbird's format. For both receiving and sending it takes
/// SSL/TLS, else STARTTLS; never an unencrypted server. IMAP when the file offers it, else
/// POP3. `None` without a usable server for either.
pub fn parse_autoconfig(xml: &str, email: &str) -> Option<Detected> {
    let (local, domain) = split(email)?;
    let xml = strip_comments(xml);
    let fill = |s: &str| {
        s.replace("%EMAILADDRESS%", email.trim())
            .replace("%EMAILLOCALPART%", local)
            .replace("%EMAILDOMAIN%", &domain)
    };
    let servers = |tag: &str, kind: &str| -> Vec<Server> {
        elements(&xml, tag)
            .into_iter()
            .filter(|(attrs, _)| attribute(attrs, "type").as_deref() == Some(kind))
            .filter_map(|(_, body)| {
                Some(Server {
                    host: fill(&text(body, "hostname")?).to_ascii_lowercase(),
                    port: text(body, "port")?.parse().ok()?,
                    socket: text(body, "socketType")?.to_ascii_uppercase(),
                    username: text(body, "username").map(|u| fill(&u)),
                })
            })
            .filter(|s| !s.host.is_empty() && s.port != 0)
            .collect()
    };
    let encrypted = |kind: &str| {
        let incoming = servers("incomingServer", kind);
        incoming
            .iter()
            .find(|s| s.socket == "SSL")
            .map(|s| (s.clone(), SmtpSecurity::Tls))
            .or_else(|| {
                incoming
                    .iter()
                    .find(|s| s.socket == "STARTTLS")
                    .map(|s| (s.clone(), SmtpSecurity::StartTls))
            })
    };
    let (imap, imap_security, pop3) = match encrypted("imap") {
        Some((server, security)) => (server, security, false),
        None => {
            let (server, security) = encrypted("pop3")?;
            (server, security, true)
        }
    };
    let imap = &imap;
    let outgoing = servers("outgoingServer", "smtp");
    let smtp = outgoing
        .iter()
        .find(|s| s.socket == "SSL")
        .map(|s| (s.host.as_str(), s.port, SmtpSecurity::Tls))
        .or_else(|| {
            outgoing
                .iter()
                .find(|s| s.socket == "STARTTLS")
                .map(|s| (s.host.as_str(), s.port, SmtpSecurity::StartTls))
        })
        .unwrap_or((imap.host.as_str(), 465, SmtpSecurity::Tls));
    let username = imap
        .username
        .clone()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| email.trim().to_owned());
    let mut found = setup(email, username, &imap.host, smtp);
    found.port = imap.port;
    found.imap_security = imap_security;
    found.pop3 = pop3.then(Pop3Keep::default);
    // The short name if there is one; some files put a slogan or the address in displayName.
    let provider = ["displayShortName", "displayName"]
        .iter()
        .filter_map(|tag| {
            elements(&xml, tag)
                .first()
                .map(|(_, name)| decode(name.trim()))
        })
        .find(|name| !name.is_empty() && !name.contains('@'));
    Some(Detected {
        provider,
        ..plain(found, Source::Autoconfig)
    })
}

#[derive(Clone)]
struct Server {
    host: String,
    port: u16,
    socket: String,
    username: Option<String>,
}

fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = rest[start..]
            .find("-->")
            .map_or("", |end| &rest[start + end + 3..]);
    }
    out.push_str(rest);
    out
}

/// Every `<tag ...>body</tag>` in `xml`, as (attributes, body).
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        // `<hostname>` must not match `<hostnameX>`.
        if !after.starts_with(|c: char| c == '>' || c.is_whitespace()) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        let attrs = &after[..gt];
        let body_and_rest = &after[gt + 1..];
        let Some(end) = body_and_rest.find(&close) else {
            break;
        };
        found.push((attrs, &body_and_rest[..end]));
        rest = &body_and_rest[end + close.len()..];
    }
    found
}

fn attribute(attrs: &str, name: &str) -> Option<String> {
    let at = attrs.find(&format!("{name}="))? + name.len() + 1;
    let quote = attrs[at..]
        .chars()
        .next()
        .filter(|q| *q == '"' || *q == '\'')?;
    let value = &attrs[at + 1..];
    Some(decode(&value[..value.find(quote)?]))
}

fn text(body: &str, tag: &str) -> Option<String> {
    let (_, inner) = *elements(body, tag).first()?;
    Some(decode(inner.trim()))
}

fn decode(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// ---- DNS: MX and SRV records ----

#[derive(Clone, Copy)]
enum RecordKind {
    Mx,
    Srv,
}

/// One MX or SRV record as the resolver hands it over. An MX record has only its preference
/// (as the priority) and its host.
#[derive(Debug, Clone, PartialEq)]
struct DnsRecord {
    priority: u16,
    weight: u16,
    host: String,
    port: u16,
}

/// Host names, with ports for SRV records, most preferred first: lowest priority, then the
/// highest weight. Lowercase, without the final dot. An SRV target of "." (the service isn't
/// offered) is left out.
fn most_preferred_first(mut records: Vec<DnsRecord>) -> Vec<(String, u16)> {
    records.sort_by_key(|r| (r.priority, std::cmp::Reverse(r.weight)));
    records
        .into_iter()
        .map(|r| (r.host.trim_end_matches('.').to_ascii_lowercase(), r.port))
        .filter(|(host, _)| !host.is_empty())
        .collect()
}

/// A name's MX or SRV records, most preferred first, within one `WAIT`. Empty when there are
/// none or the lookup fails.
async fn dns_records(name: &str, kind: RecordKind) -> Vec<(String, u16)> {
    let records = timeout(WAIT, query(name.to_owned(), kind)).await;
    most_preferred_first(records.unwrap_or_default())
}

/// On Windows, Windows' own resolver, which follows the system's DNS settings (VPNs included)
/// like every other Windows program. Its call blocks, so it runs on a thread for that.
#[cfg(windows)]
async fn query(name: String, kind: RecordKind) -> Vec<DnsRecord> {
    tokio::task::spawn_blocking(move || windows_query(&name, kind))
        .await
        .unwrap_or_default()
}

/// Elsewhere hickory-resolver, a DNS client in Rust that takes the system's DNS servers from
/// /etc/resolv.conf.
#[cfg(not(windows))]
async fn query(name: String, kind: RecordKind) -> Vec<DnsRecord> {
    use hickory_resolver::proto::rr::{RData, RecordType};
    use hickory_resolver::TokioResolver;

    let Ok(resolver) = TokioResolver::builder_tokio().and_then(|builder| builder.build()) else {
        return Vec::new();
    };
    let wanted = match kind {
        RecordKind::Mx => RecordType::MX,
        RecordKind::Srv => RecordType::SRV,
    };
    // The final dot asks for this name only, not also under the local search domains.
    let Ok(found) = resolver.lookup(format!("{name}."), wanted).await else {
        return Vec::new();
    };
    // Answers can also hold the CNAMEs that led there; only the records asked for count.
    found
        .answers()
        .iter()
        .filter_map(|record| match &record.data {
            RData::MX(mx) => Some(DnsRecord {
                priority: mx.preference,
                weight: 0,
                host: mx.exchange.to_ascii(),
                port: 0,
            }),
            RData::SRV(srv) => Some(DnsRecord {
                priority: srv.priority,
                weight: srv.weight,
                host: srv.target.to_ascii(),
                port: srv.port,
            }),
            _ => None,
        })
        .collect()
}

#[cfg(windows)]
fn windows_query(name: &str, kind: RecordKind) -> Vec<DnsRecord> {
    use std::ptr::null_mut;
    use windows_sys::Win32::NetworkManagement::Dns::{
        DnsFree, DnsFreeRecordList, DnsQuery_W, DNS_QUERY_STANDARD, DNS_RECORDA, DNS_RECORDW,
        DNS_TYPE_MX, DNS_TYPE_SRV,
    };

    let wanted = match kind {
        RecordKind::Mx => DNS_TYPE_MX,
        RecordKind::Srv => DNS_TYPE_SRV,
    };
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut first: *mut DNS_RECORDA = null_mut();
    // SAFETY: `wide` is NUL-terminated and outlives the call. On success Windows hands back a
    // list of records, which is read below and then freed once with DnsFree.
    let status = unsafe {
        DnsQuery_W(
            wide.as_ptr(),
            wanted,
            DNS_QUERY_STANDARD,
            null_mut(),
            &mut first,
            null_mut(),
        )
    };
    if status != 0 || first.is_null() {
        return Vec::new();
    }
    let mut found = Vec::new();
    // The wide-character query fills the list with DNS_RECORDW entries.
    let mut record = first.cast::<DNS_RECORDW>();
    while !record.is_null() {
        // SAFETY: `record` is an entry of the list DnsQuery_W returned, not yet freed. The
        // data union holds the record type in `wType`, whose host name is NUL-terminated or
        // null. Other entries (CNAMEs, the addresses in the additional section) are skipped.
        unsafe {
            let entry = &*record;
            if entry.wType == wanted {
                let (priority, weight, name, port) = match kind {
                    RecordKind::Mx => {
                        let mx = entry.Data.MX;
                        (mx.wPreference, 0, mx.pNameExchange, 0)
                    }
                    RecordKind::Srv => {
                        let srv = entry.Data.SRV;
                        (srv.wPriority, srv.wWeight, srv.pNameTarget, srv.wPort)
                    }
                };
                if !name.is_null() {
                    let mut len = 0;
                    while *name.add(len) != 0 {
                        len += 1;
                    }
                    let host = String::from_utf16_lossy(std::slice::from_raw_parts(name, len));
                    found.push(DnsRecord {
                        priority,
                        weight,
                        host,
                        port,
                    });
                }
            }
            record = entry.pNext;
        }
    }
    // SAFETY: frees the whole list exactly once; nothing points into it any more.
    unsafe { DnsFree(first.cast(), DnsFreeRecordList) };
    found
}

/// The domain's MX hosts, most preferred first.
pub async fn mx_hosts(domain: &str) -> Vec<String> {
    dns_records(domain, RecordKind::Mx)
        .await
        .into_iter()
        .map(|(host, _)| host)
        .collect()
}

/// The mail services a domain publishes in its DNS zone (RFC 6186 and 8314), each as
/// (host, port), most preferred first.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SrvRecords {
    /// `_imaps._tcp`: IMAP over SSL/TLS.
    pub imaps: Vec<(String, u16)>,
    /// `_pop3s._tcp`: POP3 over SSL/TLS, for a domain without IMAP.
    pub pop3s: Vec<(String, u16)>,
    /// `_submissions._tcp`: sending over SSL/TLS.
    pub submissions: Vec<(String, u16)>,
    /// `_submission._tcp`: sending with STARTTLS.
    pub submission: Vec<(String, u16)>,
}

/// Look up the domain's mail SRV records, all four at once.
pub async fn srv_records(domain: &str) -> SrvRecords {
    let lookup = |service: &str| {
        let name = format!("_{service}._tcp.{domain}");
        async move {
            dns_records(&name, RecordKind::Srv)
                .await
                .into_iter()
                .filter(|(_, port)| *port != 0)
                .collect()
        }
    };
    let (imaps, pop3s, submissions, submission) = join4(
        lookup("imaps"),
        lookup("pop3s"),
        lookup("submissions"),
        lookup("submission"),
    )
    .await;
    SrvRecords {
        imaps,
        pop3s,
        submissions,
        submission,
    }
}

/// Settings from SRV records. IMAP over SSL/TLS must be there, else POP3 over SSL/TLS;
/// sending takes SSL/TLS, else STARTTLS, else the incoming server on 465.
pub fn from_srv(email: &str, srv: &SrvRecords) -> Option<Detected> {
    split(email)?;
    let (imap, imap_port, pop3) = match srv.imaps.first() {
        Some((host, port)) => (host, port, false),
        None => srv.pop3s.first().map(|(host, port)| (host, port, true))?,
    };
    let smtp = srv
        .submissions
        .first()
        .map(|(host, port)| (host.as_str(), *port, SmtpSecurity::Tls))
        .or_else(|| {
            srv.submission
                .first()
                .map(|(host, port)| (host.as_str(), *port, SmtpSecurity::StartTls))
        })
        .unwrap_or((imap.as_str(), 465, SmtpSecurity::Tls));
    let mut found = setup(email, email.trim().to_owned(), imap, smtp);
    found.port = *imap_port;
    found.pop3 = pop3.then(Pop3Keep::default);
    Some(plain(found, Source::Srv))
}

// ---- Trying the usual names ----

/// Where an IMAP server usually is, most likely first.
fn imap_candidates(domain: &str, mx_hosts: &[String]) -> Vec<String> {
    let mut hosts = vec![format!("imap.{domain}"), format!("mail.{domain}")];
    hosts.extend(mx_hosts.iter().take(2).cloned());
    hosts.push(domain.to_owned());
    dedup(hosts)
}

/// Where an SMTP server usually is, most likely first: SSL/TLS on 465 before STARTTLS on 587.
fn smtp_candidates(domain: &str, mx_hosts: &[String]) -> Vec<(String, u16, SmtpSecurity)> {
    let mut hosts = vec![format!("smtp.{domain}"), format!("mail.{domain}")];
    hosts.extend(mx_hosts.iter().take(2).cloned());
    hosts.push(domain.to_owned());
    hosts.push(format!("imap.{domain}"));
    dedup(hosts)
        .into_iter()
        .flat_map(|h| {
            [
                (h.clone(), 465, SmtpSecurity::Tls),
                (h, 587, SmtpSecurity::StartTls),
            ]
        })
        .collect()
}

fn dedup(hosts: Vec<String>) -> Vec<String> {
    let mut seen = Vec::new();
    for h in hosts {
        if !seen.contains(&h) {
            seen.push(h);
        }
    }
    seen
}

/// A mail server answers on port 993 over TLS with a valid certificate for this name, all
/// within one `WAIT`.
async fn imap_answers(host: String) -> bool {
    let Ok(name) = ServerName::try_from(host.clone()) else {
        return false;
    };
    let attempt = async {
        let tcp = TcpStream::connect((host.as_str(), 993)).await.ok()?;
        let mut tls = crate::imap::tls_connector().connect(name, tcp).await.ok()?;
        let mut greeting = [0u8; 64];
        let n = tls.read(&mut greeting).await.ok()?;
        Some(greeting[..n].starts_with(b"* OK") || greeting[..n].starts_with(b"* PREAUTH"))
    };
    matches!(timeout(WAIT, attempt).await, Ok(Some(true)))
}

/// An SMTP server answers, with TLS and a valid certificate, within one `WAIT`. Nothing is
/// sent and no login is tried.
async fn smtp_answers(host: String, port: u16, security: SmtpSecurity) -> bool {
    let builder = match security {
        SmtpSecurity::Tls => crate::smtp::Transport::relay(&host),
        SmtpSecurity::StartTls => crate::smtp::Transport::starttls_relay(&host),
    };
    let Ok(builder) = builder else { return false };
    let transport: crate::smtp::Transport = builder.port(port).timeout(Some(WAIT)).build();
    matches!(
        timeout(WAIT, transport.test_connection()).await,
        Ok(Ok(true))
    )
}

/// Try the usual server names at once and take the most likely ones that answer.
pub async fn guess(email: &str, mx_hosts: &[String]) -> Option<Detected> {
    let (_, domain) = split(email)?;
    let imap_hosts = imap_candidates(&domain, mx_hosts);
    let smtp_options = smtp_candidates(&domain, mx_hosts);
    let (imap_ok, smtp_ok) = join(
        join_all(imap_hosts.iter().cloned().map(imap_answers)),
        join_all(
            smtp_options
                .iter()
                .cloned()
                .map(|(h, p, s)| smtp_answers(h, p, s)),
        ),
    )
    .await;
    let imap = imap_hosts
        .iter()
        .zip(imap_ok)
        .find_map(|(host, ok)| ok.then_some(host))?;
    let smtp = smtp_options
        .iter()
        .zip(smtp_ok)
        .find_map(|(option, ok)| ok.then_some(option))
        .map(|(h, p, s)| (h.as_str(), *p, *s))
        .unwrap_or((imap.as_str(), 465, SmtpSecurity::Tls));
    Some(plain(
        setup(email, email.trim().to_owned(), imap, smtp),
        Source::Guess,
    ))
}

/// The steps `detect` takes, for the wizard to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// Asking the domain's web server for its setup file and its DNS zone for SRV and MX.
    Lookup,
    /// Trying the usual server names.
    Guess,
}

/// Everything above in order. `None` when nothing was found (or the address isn't one):
/// the wizard then asks for the servers.
pub async fn detect(email: &str, progress: &(dyn Fn(Step) + Send + Sync)) -> Option<Detected> {
    let (_, domain) = split(email)?;
    if let Some(found) = known(email) {
        return Some(found);
    }
    progress(Step::Lookup);
    let (config, srv, mx) = {
        let email = email.to_owned();
        join3(
            tokio::task::spawn_blocking(move || fetch_autoconfig(&email)),
            srv_records(&domain),
            mx_hosts(&domain),
        )
        .await
    };
    let found = config
        .ok()
        .flatten()
        .or_else(|| from_srv(email, &srv))
        .or_else(|| from_mx(email, &mx));
    if found.is_some() {
        return found;
    }
    progress(Step::Guess);
    guess(email, &mx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_providers_by_domain() {
        let gmail = known("Anna.Kowalska@Gmail.com").unwrap();
        assert_eq!(gmail.provider.as_deref(), Some("Gmail"));
        assert_eq!(gmail.source, Source::List);
        assert_eq!(gmail.setup.host, "imap.gmail.com");
        assert_eq!(gmail.setup.port, 993);
        assert_eq!(gmail.setup.username, "Anna.Kowalska@Gmail.com");
        assert_eq!(gmail.setup.smtp_host, "smtp.gmail.com");
        assert_eq!(
            (gmail.setup.smtp_port, gmail.setup.smtp_security),
            (465, SmtpSecurity::Tls)
        );
        assert_eq!(gmail.note, Some(Note::AppPassword));
        assert_eq!(
            gmail.help.as_deref(),
            Some("https://myaccount.google.com/apppasswords")
        );
        assert!(gmail.unsupported.is_none());

        // WP logs in with the part before the @; Onet with the whole address, on all its domains.
        assert_eq!(
            known(" jan.nowak@wp.pl ").unwrap().setup.username,
            "jan.nowak"
        );
        let onet = known("ola@op.pl").unwrap();
        assert_eq!(onet.provider.as_deref(), Some("Onet Poczta"));
        assert_eq!(
            (onet.setup.host.as_str(), onet.setup.username.as_str()),
            ("imap.poczta.onet.pl", "ola@op.pl")
        );
        assert_eq!(known("x@tlen.pl").unwrap().setup.host, "poczta.o2.pl");
        assert_eq!(
            known("x@interia.pl").unwrap().setup.smtp_host,
            "poczta.interia.pl"
        );

        let smtp = known("x@icloud.com").unwrap().setup;
        assert_eq!(
            (smtp.smtp_port, smtp.smtp_security),
            (587, SmtpSecurity::StartTls)
        );
    }

    #[test]
    fn providers_telex_cannot_use_say_why() {
        let outlook = known("someone@hotmail.co.uk").unwrap();
        assert_eq!(outlook.provider.as_deref(), Some("Outlook.com"));
        assert_eq!(outlook.unsupported, Some(Unsupported::MicrosoftSignIn));
        assert_eq!(
            known("x@proton.me").unwrap().unsupported,
            Some(Unsupported::ProtonBridge)
        );
        assert_eq!(known("x@wp.pl").unwrap().note, Some(Note::TurnOnImap));
    }

    #[test]
    fn unknown_or_invalid_addresses() {
        assert!(known("biuro@twojastudio.pl").is_none());
        for bad in [
            "",
            "gmail.com",
            "@gmail.com",
            "a b@gmail.com",
            "x@localhost",
            "x@exa mple.com",
        ] {
            assert!(known(bad).is_none(), "{bad}");
            assert!(split(bad).is_none(), "{bad}");
        }
        // A subdomain of a provider isn't that provider.
        assert!(known("x@mail.gmail.com").is_none());
    }

    #[test]
    fn custom_domains_by_mx() {
        let mx = |hosts: &[&str]| hosts.iter().map(|h| h.to_string()).collect::<Vec<_>>();
        let workspace = from_mx(
            "biuro@studio.pl",
            &mx(&["ASPMX.L.GOOGLE.COM.", "alt1.aspmx.l.google.com"]),
        )
        .unwrap();
        assert_eq!(
            (workspace.provider.as_deref(), workspace.source),
            (Some("Gmail"), Source::Mx)
        );
        assert_eq!(workspace.setup.username, "biuro@studio.pl");
        assert_eq!(workspace.setup.host, "imap.gmail.com");

        let m365 = from_mx(
            "jan@firma.pl",
            &mx(&["firma-pl.mail.protection.outlook.com"]),
        )
        .unwrap();
        assert_eq!(m365.provider.as_deref(), Some("Microsoft 365"));
        assert_eq!(m365.unsupported, Some(Unsupported::MicrosoftSignIn));
        assert_eq!(
            from_mx("a@b.pl", &mx(&["mx1.mail.ovh.net"]))
                .unwrap()
                .setup
                .host,
            "ssl0.ovh.net"
        );
        // Purelymail's MX is not the server it tells mail programs to use.
        let pure = from_mx("a@b.pl", &mx(&["mailserver.purelymail.com."])).unwrap();
        assert_eq!(pure.provider.as_deref(), Some("Purelymail"));
        assert_eq!(
            (pure.setup.host.as_str(), pure.setup.smtp_host.as_str()),
            ("imap.purelymail.com", "smtp.purelymail.com")
        );
        assert_eq!(pure.setup.username, "a@b.pl");
        assert!(!pure.confirm);
        assert_eq!(
            from_mx("a@b.pl", &mx(&["mx.zoho.eu"]))
                .unwrap()
                .setup
                .smtp_host,
            "smtppro.zoho.eu"
        );
        // The first MX that belongs to a known provider counts; unknown ones are skipped.
        assert_eq!(
            from_mx(
                "a@b.pl",
                &mx(&["mail.b.pl", "in1-smtp.messagingengine.com"])
            )
            .unwrap()
            .setup
            .host,
            "imap.fastmail.com"
        );
        assert!(from_mx("a@b.pl", &mx(&["mail.b.pl", "notgoogle.com"])).is_none());
        assert!(from_mx("a@b.pl", &[]).is_none());
    }

    const CPANEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <!-- <incomingServer type="imap"><hostname>commented.out</hostname></incomingServer> -->
  <emailProvider id="studio.pl">
    <domain>studio.pl</domain>
    <displayName>Studio &amp; Co mail</displayName>
    <incomingServer type="pop3">
      <hostname>pop.studio.pl</hostname><port>995</port><socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>mail.%EMAILDOMAIN%</hostname><port>143</port><socketType>STARTTLS</socketType>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <incomingServer type='imap'>
      <hostname>Mail.%EMAILDOMAIN%</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILLOCALPART%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.studio.pl</hostname><port>587</port><socketType>STARTTLS</socketType>
    </outgoingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.studio.pl</hostname><port>465</port><socketType>SSL</socketType>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

    #[test]
    fn autoconfig_files_prefer_ssl() {
        let found = parse_autoconfig(CPANEL, "jan@studio.pl").unwrap();
        assert_eq!(found.source, Source::Autoconfig);
        assert_eq!(found.provider.as_deref(), Some("Studio & Co mail"));

        // A short name wins; a name that is really an address doesn't count.
        let named = CPANEL.replace(
            "<displayName>Studio &amp; Co mail</displayName>",
            "<displayName>jan@studio.pl</displayName><displayShortName>Studio</displayShortName>",
        );
        assert_eq!(
            parse_autoconfig(&named, "jan@studio.pl")
                .unwrap()
                .provider
                .as_deref(),
            Some("Studio")
        );
        let address = CPANEL
            .replace("Studio &amp; Co mail", "%EMAILADDRESS%")
            .replace("<displayName>%EMAILADDRESS%", "<displayName>jan@studio.pl");
        assert_eq!(
            parse_autoconfig(&address, "jan@studio.pl")
                .unwrap()
                .provider,
            None
        );
        assert_eq!(
            (found.setup.host.as_str(), found.setup.port),
            ("mail.studio.pl", 993)
        );
        assert_eq!(found.setup.username, "jan");
        assert_eq!(found.setup.smtp_host, "smtp.studio.pl");
        assert_eq!(
            (found.setup.smtp_port, found.setup.smtp_security),
            (465, SmtpSecurity::Tls)
        );
    }

    #[test]
    fn autoconfig_takes_starttls_when_there_is_no_ssl() {
        let starttls_smtp = CPANEL.replace(
            "<port>465</port><socketType>SSL</socketType>",
            "<port>2525</port><socketType>plain</socketType>",
        );
        let found = parse_autoconfig(&starttls_smtp, "jan@studio.pl").unwrap();
        assert_eq!(
            (found.setup.smtp_port, found.setup.smtp_security),
            (587, SmtpSecurity::StartTls)
        );

        // IMAP only over STARTTLS: the first such server, on its port.
        let starttls_imap = CPANEL.replace(
            "<socketType>SSL</socketType>\n      <username>%EMAILLOCALPART%",
            "<socketType>STARTTLS</socketType>\n      <username>%EMAILLOCALPART%",
        );
        let found = parse_autoconfig(&starttls_imap, "jan@studio.pl").unwrap();
        assert_eq!(
            (
                found.setup.host.as_str(),
                found.setup.port,
                found.setup.imap_security
            ),
            ("mail.studio.pl", 143, SmtpSecurity::StartTls)
        );
        // Never an unencrypted one.
        let plain_imap = starttls_imap.replace(
            "<socketType>STARTTLS</socketType>\n      <username>%EMAIL",
            "<socketType>plain</socketType>\n      <username>%EMAIL",
        );
        // Then the file's POP3 server, encrypted.
        let pop3 = parse_autoconfig(&plain_imap, "jan@studio.pl")
            .unwrap()
            .setup;
        assert_eq!(
            (pop3.host.as_str(), pop3.port, pop3.imap_security),
            ("pop.studio.pl", 995, SmtpSecurity::Tls)
        );
        assert_eq!(pop3.pop3, Some(Pop3Keep::default()), "left on the server");
        assert_eq!(pop3.username, "jan@studio.pl");
        let no_pop3 = plain_imap.replace(
            "<port>995</port><socketType>SSL</socketType>",
            "<port>110</port><socketType>plain</socketType>",
        );
        assert!(parse_autoconfig(&no_pop3, "jan@studio.pl").is_none());
        assert!(parse_autoconfig("<html>Not found</html>", "jan@studio.pl").is_none());
        // With both, IMAP.
        assert!(parse_autoconfig(CPANEL, "jan@studio.pl")
            .unwrap()
            .setup
            .pop3
            .is_none());
    }

    #[test]
    fn autoconfig_addresses() {
        let [first, second] = autoconfig_urls("Jan+news@Studio.pl", "studio.pl");
        assert_eq!(
            first,
            "https://autoconfig.studio.pl/mail/config-v1.1.xml?emailaddress=Jan%2Bnews%40Studio.pl"
        );
        assert_eq!(
            second,
            "https://studio.pl/.well-known/autoconfig/mail/config-v1.1.xml"
        );
    }

    #[test]
    fn dns_zone_srv_records() {
        let records = |list: &[(&str, u16)]| {
            list.iter()
                .map(|(h, p)| (h.to_string(), *p))
                .collect::<Vec<_>>()
        };
        let srv = SrvRecords {
            imaps: records(&[("imap.studio.pl", 993)]),
            pop3s: records(&[("pop.studio.pl", 995)]),
            submissions: records(&[("smtp.studio.pl", 465)]),
            submission: records(&[("smtp.studio.pl", 587)]),
        };
        let found = from_srv("jan@studio.pl", &srv).unwrap();
        assert_eq!(found.source, Source::Srv);
        assert!(!found.confirm, "servers in the address's own domain");
        // A server elsewhere, named only by DNS: the wizard asks before the password goes.
        let elsewhere = SrvRecords {
            imaps: records(&[("mail.hosting-firma.com", 993)]),
            ..srv.clone()
        };
        assert!(from_srv("jan@studio.pl", &elsewhere).unwrap().confirm);
        assert_eq!(
            (found.setup.host.as_str(), found.setup.port),
            ("imap.studio.pl", 993)
        );
        assert_eq!(
            (found.setup.smtp_port, found.setup.smtp_security),
            (465, SmtpSecurity::Tls)
        );
        assert_eq!(found.setup.username, "jan@studio.pl");

        // Only STARTTLS for sending, on a port of the domain's choosing.
        let starttls = SrvRecords {
            submissions: Vec::new(),
            submission: records(&[("out.studio.pl", 2587)]),
            ..srv.clone()
        };
        let found = from_srv("jan@studio.pl", &starttls).unwrap();
        assert_eq!(
            (
                found.setup.smtp_host.as_str(),
                found.setup.smtp_port,
                found.setup.smtp_security
            ),
            ("out.studio.pl", 2587, SmtpSecurity::StartTls)
        );

        assert!(found.setup.pop3.is_none(), "IMAP before POP3");

        // No IMAP over TLS in the zone: POP3 over TLS, else SRV records don't settle it.
        let pop3 = from_srv(
            "jan@studio.pl",
            &SrvRecords {
                imaps: Vec::new(),
                ..srv.clone()
            },
        )
        .unwrap()
        .setup;
        assert_eq!((pop3.host.as_str(), pop3.port), ("pop.studio.pl", 995));
        assert_eq!(pop3.pop3, Some(Pop3Keep::default()));
        assert!(from_srv(
            "jan@studio.pl",
            &SrvRecords {
                imaps: Vec::new(),
                pop3s: Vec::new(),
                ..srv
            }
        )
        .is_none());
        assert!(from_srv("jan@studio.pl", &SrvRecords::default()).is_none());
    }

    #[test]
    fn usual_names_in_order() {
        let mx = vec![
            "s12.hosting.pl".to_string(),
            "s13.hosting.pl".to_string(),
            "s14.hosting.pl".to_string(),
        ];
        assert_eq!(
            imap_candidates("studio.pl", &mx),
            [
                "imap.studio.pl",
                "mail.studio.pl",
                "s12.hosting.pl",
                "s13.hosting.pl",
                "studio.pl"
            ]
        );
        let smtp = smtp_candidates("studio.pl", &["mail.studio.pl".to_string()]);
        let names: Vec<String> = smtp.iter().map(|(h, p, _)| format!("{h}:{p}")).collect();
        assert_eq!(
            names,
            [
                "smtp.studio.pl:465",
                "smtp.studio.pl:587",
                "mail.studio.pl:465",
                "mail.studio.pl:587",
                "studio.pl:465",
                "studio.pl:587",
                "imap.studio.pl:465",
                "imap.studio.pl:587",
            ]
        );
        assert_eq!(smtp[1].2, SmtpSecurity::StartTls);
    }

    /// The wizard in ui/app.js reads these names and codes.
    #[test]
    fn crosses_the_bridge_as_the_ui_expects() {
        let wp = serde_json::to_value(known("jan@wp.pl").unwrap()).unwrap();
        assert_eq!(wp["source"], "list");
        assert_eq!(wp["provider"], "WP Poczta");
        assert_eq!(wp["note"], "turn_on_imap");
        assert_eq!(wp["setup"]["username"], "jan");
        assert_eq!(wp["setup"]["smtp_host"], "smtp.wp.pl");
        assert_eq!(wp["setup"]["smtp_security"], "tls");
        let outlook = serde_json::to_value(known("a@outlook.com").unwrap()).unwrap();
        assert_eq!(outlook["unsupported"], "microsoft_sign_in");
        assert_eq!(
            serde_json::to_value(Note::TurnOnImapAndAppPassword).unwrap(),
            "turn_on_imap_and_app_password"
        );
        assert_eq!(
            serde_json::to_value(Unsupported::ProtonBridge).unwrap(),
            "proton_bridge"
        );
        assert_eq!(serde_json::to_value(Source::Srv).unwrap(), "srv");
        assert_eq!(serde_json::to_value(Step::Guess).unwrap(), "guess");
    }

    #[tokio::test]
    async fn known_providers_need_no_network() {
        let steps = std::sync::Mutex::new(Vec::new());
        let found = detect("ola@o2.pl", &|s| steps.lock().unwrap().push(s))
            .await
            .unwrap();
        assert_eq!(found.setup.username, "ola");
        assert!(steps.lock().unwrap().is_empty());
        assert!(detect("not an address", &|_| {}).await.is_none());
    }

    #[test]
    fn dns_records_most_preferred_first() {
        let record = |priority, weight, host: &str, port| DnsRecord {
            priority,
            weight,
            host: host.to_string(),
            port,
        };
        let found = most_preferred_first(vec![
            record(20, 0, "ALT2.ASPMX.L.GOOGLE.COM.", 0),
            record(10, 5, "light.studio.pl.", 993),
            record(10, 60, "heavy.studio.pl", 993),
            record(0, 0, ".", 0),
            record(5, 0, "", 0),
        ]);
        assert_eq!(
            found,
            [
                ("heavy.studio.pl".to_string(), 993),
                ("light.studio.pl".to_string(), 993),
                ("alt2.aspmx.l.google.com".to_string(), 0),
            ]
        );
        assert!(most_preferred_first(Vec::new()).is_empty());
    }

    /// Needs internet: `cargo test -p telex-core -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn guesses_a_real_server() {
        let found = guess("someone@gmail.com", &[]).await.unwrap();
        assert_eq!(found.setup.host, "imap.gmail.com");
        assert_eq!(
            (found.setup.smtp_host.as_str(), found.setup.smtp_port),
            ("smtp.gmail.com", 465)
        );
    }

    /// Needs internet: Windows' resolver there, hickory-resolver elsewhere.
    #[tokio::test]
    #[ignore]
    async fn looks_up_real_dns_records() {
        let mx = mx_hosts("gmail.com").await;
        assert!(mx.iter().any(|h| h.ends_with("google.com")), "{mx:?}");
        let srv = srv_records("gmail.com").await;
        assert_eq!(
            srv.imaps.first(),
            Some(&("imap.gmail.com".to_string(), 993))
        );
        assert_eq!(
            srv.submission.first(),
            Some(&("smtp.gmail.com".to_string(), 587))
        );
        assert!(mx_hosts("no-such-domain.invalid").await.is_empty());
    }
}
