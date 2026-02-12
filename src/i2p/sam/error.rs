use std::fmt;
use std::time::Duration;

/// Domain-specific errors for SAM (Simple Anonymous Messaging) interactions.
///
/// Pattern:
/// - Lower-level SAM modules return `SamError` (typed).
/// - Higher-level code should preserve `SamError` variants so reconnect/retry decisions can match
///   on the typed error directly.
#[derive(Debug)]
pub enum SamError {
    /// The SAM TCP connection was closed by the remote side.
    Closed,

    /// A read/write operation exceeded the configured timeout.
    Timeout { op: String, dur: Duration },

    /// Underlying I/O error during a SAM operation.
    Io { op: String, source: std::io::Error },

    /// The SAM bridge sent a syntactically invalid frame/reply.
    BadFrame { what: &'static str, raw: String },

    /// The TCP-DATAGRAM stream framing is out of sync (payload bytes were read as header lines).
    ///
    /// This is not recoverable in-place; the only safe recovery is to reconnect the session.
    FramingDesync { what: &'static str, details: String },

    /// A protocol invariant was violated by our caller (e.g. sending before HELLO/SESSION).
    Protocol { what: String },

    /// SAM replied with `RESULT != OK`.
    ReplyError {
        verb: String,
        kind: String,
        result: Option<String>,
        message: Option<String>,
        raw_redacted: String,
    },
}

impl SamError {
    pub fn io(op: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            op: op.into(),
            source,
        }
    }

    pub fn timeout(op: impl Into<String>, dur: Duration) -> Self {
        Self::Timeout { op: op.into(), dur }
    }

    pub fn protocol(what: impl Into<String>) -> Self {
        Self::Protocol { what: what.into() }
    }
}

impl fmt::Display for SamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SamError::Closed => write!(f, "SAM closed the connection"),
            SamError::Timeout { op, dur } => write!(f, "SAM {op} timed out after {dur:?}"),
            SamError::Io { op, source } => write!(f, "SAM {op} I/O error: {source}"),
            SamError::BadFrame { what, raw } => write!(f, "Bad SAM frame ({what}): {raw}"),
            SamError::FramingDesync { what, details } => {
                write!(f, "SAM framing out of sync ({what}): {details}")
            }
            SamError::Protocol { what } => write!(f, "SAM protocol error: {what}"),
            SamError::ReplyError {
                verb,
                kind,
                result,
                message,
                raw_redacted,
            } => write!(
                f,
                "SAM error: verb={verb} kind={kind} result={result:?} message={message:?} raw={raw_redacted}",
            ),
        }
    }
}

impl std::error::Error for SamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SamError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
