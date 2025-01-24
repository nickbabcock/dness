use crate::errors::{DnsError, DnsErrorKind};
use std::net::{IpAddr, Ipv4Addr};
use hickory_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;

#[derive(Debug)]
pub struct DnsResolver {
    resolver: TokioAsyncResolver,
}

impl DnsResolver {
    pub async fn create_opendns() -> Result<Self, DnsError> {
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
                false,
            ),
        );

        Self::from_config(config).await
    }

    pub async fn create_cloudflare() -> Result<Self, DnsError> {
        Self::from_config(ResolverConfig::cloudflare()).await
    }

    pub async fn from_config(config: ResolverConfig) -> Result<Self, DnsError> {
        let resolver = TokioAsyncResolver::tokio(config, ResolverOpts::default());

        Ok(DnsResolver { resolver })
    }

    pub async fn ipv4_lookup(&self, host: &str) -> Result<Ipv4Addr, DnsError> {
        // When we query opendns for the special domain of "myip.opendns.com" it will return to us
        // our IP
        let response = self
            .resolver
            .ipv4_lookup(host)
            .await
            .map_err(|e| DnsError {
                kind: Box::new(DnsErrorKind::DnsResolve(e)),
            })?;

        response
            .iter()
            .next()
            .map(|address| address.0)
            .ok_or_else(|| DnsError {
                kind: Box::new(DnsErrorKind::UnexpectedResponse(0)),
            })
    }
}

#[derive(Debug)]
struct OpenDnsResolver {
    resolver: DnsResolver,
}

impl OpenDnsResolver {
    async fn create() -> Result<Self, DnsError> {
        let resolver = DnsResolver::create_opendns().await?;
        Ok(OpenDnsResolver { resolver })
    }

    async fn wan_lookup(&self) -> Result<Ipv4Addr, DnsError> {
        self.resolver.ipv4_lookup("myip.opendns.com.").await
    }
}

pub async fn wan_lookup_ip() -> Result<Ipv4Addr, DnsError> {
    let opendns = OpenDnsResolver::create().await?;
    opendns.wan_lookup().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opendns_lookup_ip_test() {
        // Heads up: this test requires internet connectivity
        match wan_lookup_ip().await {
            Ok(ip) => assert!(ip != Ipv4Addr::new(127, 0, 0, 1)),
            Err(e) => {
                match e.kind.as_ref() {
                    DnsErrorKind::DnsResolve(e) => {
                        match e.kind() {
                            hickory_resolver::error::ResolveErrorKind::NoRecordsFound {
                                ..
                            } => {
                                // This is fine, just means we're behind a CGNAT
                            }
                            _ => panic!("unexpected error: {}", e),
                        }
                    }
                    DnsErrorKind::UnexpectedResponse(_) => {
                        panic!("unexpected response: {}", e);
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn cloudflare_test() {
        // Heads up: this test requires internet connectivity
        let resolver = DnsResolver::create_cloudflare().await.unwrap();
        let ip = resolver.ipv4_lookup("example.com.").await.unwrap();
        assert!(ip != Ipv4Addr::new(127, 0, 0, 1));
    }
}
