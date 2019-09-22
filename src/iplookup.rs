use failure::{Compat, Fail};
use std::error;
use std::fmt;
use std::io::Error as IoError;
use std::net::{IpAddr, Ipv4Addr};
use trust_dns_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use trust_dns_resolver::error::ResolveError;
use trust_dns_resolver::Resolver;

#[derive(Debug)]
pub struct LookupError {
    kind: LookupErrorKind,
}

#[derive(Debug)]
pub enum LookupErrorKind {
    DnsCreation(IoError),
    DnsResolve(Compat<ResolveError>),
    UnexpectedResponse(usize),
}

impl error::Error for LookupError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            LookupErrorKind::DnsCreation(ref e) => Some(e),
            LookupErrorKind::DnsResolve(ref e) => Some(e),
            LookupErrorKind::UnexpectedResponse(_) => None,
        }
    }
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resolving WAN IP issue: ")?;
        match self.kind {
            LookupErrorKind::DnsCreation(ref _e) => write!(f, "could not create dns resolver"),
            LookupErrorKind::DnsResolve(ref _e) => write!(f, "could not resolve via dns"),
            LookupErrorKind::UnexpectedResponse(results) => {
                write!(f, "unexpected number of results: {}", results)
            }
        }
    }
}

struct OpenDnsLookup {
    resolver: Resolver,
}

impl OpenDnsLookup {
    fn create() -> Result<Self, LookupError> {
        let config = ResolverConfig::from_parts(
            None,
            vec![],
            NameServerConfigGroup::from_ips_clear(
                &[
                    // OpenDNS nameservers:
                    // https://en.wikipedia.org/wiki/OpenDNS#Name_server_IP_addresses
                    IpAddr::V4(Ipv4Addr::new(208, 67, 222, 222)),
                    IpAddr::V4(Ipv4Addr::new(208, 67, 220, 220)),
                ],
                53,
            ),
        );
        let resolver = Resolver::new(config, ResolverOpts::default()).map_err(|e| LookupError {
            kind: LookupErrorKind::DnsCreation(e),
        })?;
        Ok(OpenDnsLookup { resolver })
    }

    fn lookup(&self) -> Result<Ipv4Addr, LookupError> {
        // When we query opendns for the special domain of "myip.opendns.com" it will return to us
        // our IP
        let response = self
            .resolver
            .ipv4_lookup("myip.opendns.com.")
            .map_err(|e| LookupError {
                kind: LookupErrorKind::DnsResolve(e.compat()),
            })?;

        // If we get anything other than 1 address back, it's an error
        let addresses: Vec<Ipv4Addr> = response.iter().cloned().collect();
        if addresses.len() != 1 {
            Err(LookupError {
                kind: LookupErrorKind::UnexpectedResponse(addresses.len()),
            })
        } else {
            Ok(addresses[0])
        }
    }
}

pub fn lookup_ip() -> Result<Ipv4Addr, LookupError> {
    OpenDnsLookup::create()?.lookup()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opendns_lookup_ip_test() {
        // Heads up: this test requires internet connectivity
        let ip = lookup_ip().unwrap();
        assert!(ip != Ipv4Addr::new(127, 0, 0, 1));
    }
}
