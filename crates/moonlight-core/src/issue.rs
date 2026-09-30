//! Something the user needs to know went wrong — as a kind, not a sentence.
//!
//! The app words each case itself, in the user's language. Errors used to reach
//! the screen as their own English descriptions, which named the service behind
//! the subscription ("Panel returned HTTP 502") and could carry a link or a
//! server address. Those descriptions still go to the log, redacted (see
//! [`crate::redact`]); the screen only ever gets one of these.

use crate::subscription::Failure;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    /// What was pasted is not a subscription link.
    InvalidLink,
    /// Connecting with no subscription.
    NoSubscription,
    /// The subscription server did not answer properly — down, overloaded or
    /// unreachable. The HTTP status, when there was one, is kept for support.
    ServerUnavailable(Option<u16>),
    /// The link was refused: revoked, mistyped, or the plan was deleted.
    LinkRejected,
    /// The subscription answered with nothing in it.
    EmptySubscription,
    /// It has servers, but none this app can use.
    NoUsableServers,
    /// The account is at its device limit, with the service's own explanation
    /// when it sent one — the one text from the service shown verbatim, since
    /// it is written for users.
    DeviceLimit(Option<String>),
    /// The service wants a device identifier this request could not give.
    DeviceNotSupported,
    /// The core would not start, or would not take the config.
    CoreFailed,
    /// Another VPN or proxy already owns the system routes TUN needs.
    RoutesTaken,
    /// The TUN interface could not be created for any other reason.
    TunFailed,
    /// TUN needs the helper service and it is missing or not answering.
    HelperMissing,
    /// The core would not take the config the user's rules make, so nothing
    /// was changed.
    RulesRefused,
}

impl From<&Failure> for Issue {
    fn from(failure: &Failure) -> Issue {
        match failure {
            Failure::BadUrl => Issue::InvalidLink,
            Failure::Http(401 | 403 | 404 | 410) => Issue::LinkRejected,
            Failure::Http(code) => Issue::ServerUnavailable(Some(*code)),
            Failure::Transport(_) => Issue::ServerUnavailable(None),
            Failure::Empty => Issue::EmptySubscription,
            Failure::Unusable(_) => Issue::NoUsableServers,
            Failure::DeviceLimit(message) => Issue::DeviceLimit(message.clone()),
            Failure::DeviceNotSupported => Issue::DeviceNotSupported,
        }
    }
}
