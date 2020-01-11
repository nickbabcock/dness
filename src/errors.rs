use failure::Compat;
use std::error;
use std::fmt;
use trust_dns_resolver::error::ResolveError;

#[derive(Debug)]
pub enum DnessErrorKind {
    SendHttp {
        url: String,
        context: String,
        source: reqwest::Error,
    },
    BadResponse {
        url: String,
        context: String,
        source: reqwest::Error,
    },
    Deserialize {
        url: String,
        context: String,
        source: reqwest::Error,
    },
    Message(String),
    Dns {
        source: DnsError,
    },
}

#[derive(Debug)]
pub struct DnessError {
    kind: DnessErrorKind,
}

impl DnessError {
    pub fn send_http(url: &str, context: &str, source: reqwest::Error) -> DnessError {
        DnessError {
            kind: DnessErrorKind::SendHttp {
                url: String::from(url),
                context: String::from(context),
                source,
            },
        }
    }

    pub fn bad_response(url: &str, context: &str, source: reqwest::Error) -> DnessError {
        DnessError {
            kind: DnessErrorKind::BadResponse {
                url: String::from(url),
                context: String::from(context),
                source,
            },
        }
    }

    pub fn deserialize(url: &str, context: &str, source: reqwest::Error) -> DnessError {
        DnessError {
            kind: DnessErrorKind::Deserialize {
                url: String::from(url),
                context: String::from(context),
                source,
            },
        }
    }

    pub fn message(msg: String) -> DnessError {
        DnessError {
            kind: DnessErrorKind::Message(msg),
        }
    }
}

impl From<DnsError> for DnessError {
    fn from(source: DnsError) -> Self {
        DnessError {
            kind: DnessErrorKind::Dns { source },
        }
    }
}

impl error::Error for DnessError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            DnessErrorKind::SendHttp { ref source, .. } => Some(source),
            DnessErrorKind::BadResponse { ref source, .. } => Some(source),
            DnessErrorKind::Deserialize { ref source, .. } => Some(source),
            DnessErrorKind::Dns { ref source, .. } => Some(source),
            _ => None,
        }
    }
}

impl fmt::Display for DnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DnessErrorKind::SendHttp { url, context, .. } => write!(
                f,
                "unable to send http request for {}: url attempted: {}",
                context, url
            ),
            DnessErrorKind::BadResponse { url, context, .. } => write!(
                f,
                "received bad http response for {}: url attempted: {}",
                context, url
            ),
            DnessErrorKind::Deserialize { url, context, .. } => write!(
                f,
                "unable to deserialize response for {}: url attempted: {}",
                context, url
            ),
            DnessErrorKind::Dns { .. } => write!(f, "dns lookup"),
            DnessErrorKind::Message(msg) => write!(f, "{}", msg),
        }
    }
}

#[derive(Debug)]
pub struct DnsError {
    pub kind: DnsErrorKind,
}

#[derive(Debug)]
pub enum DnsErrorKind {
    DnsCreation(Compat<ResolveError>),
    DnsResolve(Compat<ResolveError>),
    UnexpectedResponse(usize),
}

impl error::Error for DnsError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            DnsErrorKind::DnsCreation(ref e) => Some(e),
            DnsErrorKind::DnsResolve(ref e) => Some(e),
            DnsErrorKind::UnexpectedResponse(_) => None,
        }
    }
}

impl fmt::Display for DnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DnsErrorKind::DnsCreation(_) => write!(f, "could not create dns resolver"),
            DnsErrorKind::DnsResolve(_) => write!(f, "could not resolve via dns"),
            DnsErrorKind::UnexpectedResponse(results) => {
                write!(f, "unexpected number of results: {}", results)
            }
        }
    }
}
