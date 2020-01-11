use crate::config::NamecheapConfig;
use crate::core::Updates;
use crate::dns::DnsResolver;
use crate::errors::DnessError;
use log::warn;
use std::net::Ipv4Addr;

#[derive(Debug)]
pub struct NamecheapProvider<'a> {
    client: &'a reqwest::Client,
    config: &'a NamecheapConfig,
}

impl<'a> NamecheapProvider<'a> {
    pub async fn update_domain(&self, host: &str, wan: Ipv4Addr) -> Result<(), DnessError> {
        let base = self.config.base_url.trim_end_matches('/').to_string();
        let get_url = format!("{}/update", base);
        let response = self
            .client
            .get(&get_url)
            .query(&[
                ("host", host),
                ("domain", &self.config.domain),
                ("password", &self.config.ddns_password),
                ("ip", &wan.to_string()),
            ])
            .send()
            .await
            .map_err(|e| DnessError::send_http(&get_url, "namecheap update", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&get_url, "namecheap update", e))?
            .text()
            .await
            .map_err(|e| DnessError::deserialize(&get_url, "namecheap update", e))?;

        if !response.contains("<ErrCount>0</ErrCount>") {
            Err(DnessError::message(format!(
                "expected zero errors, but received: {}",
                response
            )))
        } else {
            Ok(())
        }
    }
}

pub async fn update_domains(
    client: &reqwest::Client,
    config: &NamecheapConfig,
    wan: Ipv4Addr,
) -> Result<Updates, DnessError> {
    // Use cloudflare's DNS over opendns so that we can query the records over tls. Opendns doesn't
    // seem to support dns over tls yet. We're going to be using dns to check all the records
    // listed in the config so that we can be a good netizen and needlessly send update requests to
    // namecheap's servers.
    let resolver = DnsResolver::create_cloudflare_tls().await?;
    let namecheap = NamecheapProvider { client, config };

    let mut results = Updates::default();
    let queries = config
        .records
        .iter()
        .map(|x| format!("{}.{}.", x, config.domain));
    for record in queries {
        let response = resolver.ipv4_lookup(&record).await;

        match response {
            Ok(ip) => {
                if ip == wan {
                    results.current += 1;
                } else {
                    namecheap.update_domain(&record, wan).await?;
                    results.updated += 1;
                }
            }
            Err(e) => {
                // Could be a network issue or it could be that the record didn't exist.
                warn!(
                    "resolving namecheap record ({}) encountered an error: {}",
                    record, e
                );
                results.missing += 1;
            }
        }
    }

    Ok(Updates::default())
}
