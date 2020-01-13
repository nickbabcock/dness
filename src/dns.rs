use crate::errors::{DnsError, DnsErrorKind};
use failure::Fail;
use std::net::{IpAddr, Ipv4Addr};
use tokio::runtime::Handle;
use trust_dns_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use trust_dns_resolver::TokioAsyncResolver;

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
            ),
        );

        Self::from_config(config).await
    }

    pub async fn create_cloudflare() -> Result<Self, DnsError> {
        Self::from_config(ResolverConfig::cloudflare()).await
    }

    pub async fn from_config(config: ResolverConfig) -> Result<Self, DnsError> {
        let resolver = TokioAsyncResolver::new(config, ResolverOpts::default(), Handle::current())
            .await
            .map_err(|e| DnsError {
                kind: DnsErrorKind::DnsCreation(e.compat()),
            })?;

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
                kind: DnsErrorKind::DnsResolve(e.compat()),
            })?;

        // If we get anything other than 1 address back, it's an error
        let addresses: Vec<Ipv4Addr> = response.iter().cloned().collect();
        if addresses.len() != 1 {
            Err(DnsError {
                kind: DnsErrorKind::UnexpectedResponse(addresses.len()),
            })
        } else {
            Ok(addresses[0])
        }
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
        let ip = wan_lookup_ip().await.unwrap();
        assert!(ip != Ipv4Addr::new(127, 0, 0, 1));
    }

    #[tokio::test]
    async fn cloudflare_test() {
        // Heads up: this test requires internet connectivity
        let resolver = DnsResolver::create_cloudflare().await.unwrap();
        let ip = resolver.ipv4_lookup("example.com.").await.unwrap();
        assert!(ip != Ipv4Addr::new(127, 0, 0, 1));
    }
}
