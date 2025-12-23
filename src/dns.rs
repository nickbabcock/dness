use crate::config::IpType;
use crate::errors::{DnsError, DnsErrorKind};
use hickory_resolver::config::{NameServerConfigGroup, ResolverConfig};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::TokioResolver;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug)]
pub struct DnsResolver {
    resolver: TokioResolver,
}

impl DnsResolver {
    pub async fn create_opendns(ip_type: IpType) -> Result<Self, DnsError> {
        let ips = // OpenDNS nameservers:
                // https://en.wikipedia.org/wiki/OpenDNS#Name_server_IP_addresses
                match ip_type {
                    IpType::V4 => [
                        IpAddr::V4(Ipv4Addr::new(208, 67, 222, 222)),
                        IpAddr::V4(Ipv4Addr::new(208, 67, 220, 220)),
                    ],
                    IpType::V6 => [
                        IpAddr::V6(Ipv6Addr::new(0x2620, 0x119, 0x35, 0, 0, 0, 0, 0x35)),
                        IpAddr::V6(Ipv6Addr::new(0x2620, 0x119, 0x53, 0, 0, 0, 0, 0x53)),
                    ],
                };

        let config = ResolverConfig::from_parts(
            None,
            vec![],
            NameServerConfigGroup::from_ips_clear(&ips, 53, false),
        );

        Self::from_config(config).await
    }

    pub async fn create_cloudflare() -> Result<Self, DnsError> {
        Self::from_config(ResolverConfig::cloudflare()).await
    }

    pub async fn from_config(config: ResolverConfig) -> Result<Self, DnsError> {
        let resolver = TokioResolver::builder_with_config(config, TokioConnectionProvider::default())
            .build();

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

    pub async fn ipv6_lookup(&self, host: &str) -> Result<Ipv6Addr, DnsError> {
        // When we query opendns for the special domain of "myip.opendns.com" it will return to us
        // our IP
        let response = self
            .resolver
            .ipv6_lookup(host)
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

    pub async fn ip_lookup(&self, host: &str, ip_type: IpType) -> Result<IpAddr, DnsError> {
        Ok(match ip_type {
            IpType::V4 => self.ipv4_lookup(host).await?.into(),
            IpType::V6 => self.ipv6_lookup(host).await?.into(),
        })
    }
}

#[derive(Debug)]
struct OpenDnsResolver {
    resolver: DnsResolver,
    ip_type: IpType,
}

impl OpenDnsResolver {
    async fn create(ip_type: IpType) -> Result<Self, DnsError> {
        let resolver = DnsResolver::create_opendns(ip_type).await?;
        Ok(OpenDnsResolver { resolver, ip_type })
    }

    async fn wan_lookup(&self) -> Result<IpAddr, DnsError> {
        const DOMAIN: &str = "myip.opendns.com.";
        match self.ip_type {
            IpType::V4 => self.resolver.ipv4_lookup(DOMAIN).await.map(Into::into),
            IpType::V6 => self.resolver.ipv6_lookup(DOMAIN).await.map(Into::into),
        }
    }
}

pub async fn wan_lookup_ip(ip_type: IpType) -> Result<IpAddr, DnsError> {
    let opendns = OpenDnsResolver::create(ip_type).await?;
    opendns.wan_lookup().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn opendns_lookup_ipv4_test() {
        // Heads up: this test requires internet connectivity
        match wan_lookup_ip(IpType::V4).await {
            Ok(ip) => {
                assert!(ip.is_ipv4());
                assert!(!ip.is_loopback());
            }
            Err(e) => {
                match e.kind.as_ref() {
                    DnsErrorKind::DnsResolve(e) => {
                        // Check if this is a "no records found" error (e.g., CGNAT scenario)
                        if let hickory_resolver::ResolveErrorKind::Proto(proto_err) = e.kind() {
                            if proto_err.is_no_records_found() {
                                // This is fine, just means we're behind a CGNAT
                                return;
                            }
                        }
                        panic!("unexpected DNS error: {}", e);
                    }
                    DnsErrorKind::UnexpectedResponse(_) => {
                        panic!("unexpected response: {}", e);
                    }
                }
            }
        }
    }

    #[tokio::test]
    #[ignore] // GitHub runner doesn't have IPv6 internet connectivity
    async fn opendns_lookup_ipv6_test() {
        // Heads up: this test requires internet connectivity
        match wan_lookup_ip(IpType::V6).await {
            Ok(ip) => {
                assert!(ip.is_ipv6());
                assert!(!ip.is_loopback());
            }
            Err(e) => {
                match e.kind.as_ref() {
                    DnsErrorKind::DnsResolve(e) => {
                        // Check if this is a "no records found" error (e.g., CGNAT scenario)
                        if let hickory_resolver::ResolveErrorKind::Proto(proto_err) = e.kind() {
                            if proto_err.is_no_records_found() {
                                // This is fine, just means we're behind a CGNAT
                                return;
                            }
                        }
                        panic!("unexpected DNS error: {}", e);
                    }
                    DnsErrorKind::UnexpectedResponse(_) => {
                        panic!("unexpected response: {}", e);
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn cloudflare_lookup_ipv4_test() {
        // Heads up: this test requires internet connectivity
        let resolver = DnsResolver::create_cloudflare().await.unwrap();
        let ip = resolver.ipv4_lookup("example.com.").await.unwrap();
        assert!(!ip.is_loopback());
    }

    #[tokio::test]
    #[ignore] // GitHub runner doesn't have IPv6 internet connectivity
    async fn cloudflare_lookup_ipv6_test() {
        // Heads up: this test requires internet connectivity
        let resolver = DnsResolver::create_cloudflare().await.unwrap();
        let ip = resolver.ipv6_lookup("example.com.").await.unwrap();
        assert!(!ip.is_loopback());
    }
}
