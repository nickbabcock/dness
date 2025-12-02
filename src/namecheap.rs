use crate::config::NamecheapConfig;
use crate::core::Updates;
use crate::dns::DnsResolver;
use crate::errors::DnessError;
use log::{info, warn};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug)]
pub struct NamecheapProvider<'a> {
    client: &'a reqwest::Client,
    config: &'a NamecheapConfig,
}

impl NamecheapProvider<'_> {
    /// https://www.namecheap.com/support/knowledgebase/article.aspx/29/11/how-do-i-use-a-browser-to-dynamically-update-the-hosts-ip
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
    wan: IpAddr,
) -> Result<Updates, DnessError> {
    // Use cloudflare's DNS to query all the configured records. Ideally we'd use dns
    // over tls for privacy purposes but that feature is experimental and we don't want to rely on
    // experimental features here: https://github.com/bluejekyll/trust-dns/issues/989
    //
    // We check all the records with DNS before issuing any requests to update them in namecheap so
    // that we can be a good netizen. One issue seen with this approach is that in subsequent
    // invocations (cron, timers, etc) -- the dns record won't have propagated yet. I haven't seen
    // any issues with setting the namecheap record to an unchanged value, but it is less than
    // ideal. Namecheap does have a dns api that may be worth exploring.
    let IpAddr::V4(wan) = wan else {
        unimplemented!("IPv6 not supported for Namecheap")
    };
    let resolver = DnsResolver::create_cloudflare().await?;
    let namecheap = NamecheapProvider { client, config };

    let mut results = Updates::default();

    for record in &config.records {
        let dns_query = if record == "@" {
            format!("{}.", config.domain)
        } else {
            format!("{}.{}.", record, config.domain)
        };

        let response = resolver.ipv4_lookup(&dns_query).await;

        match response {
            Ok(ip) => {
                if ip == wan {
                    results.current += 1;
                } else {
                    namecheap.update_domain(record, wan).await?;
                    info!(
                        "{} from domain {} updated from {} to {}",
                        record, config.domain, ip, wan
                    );
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

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! namecheap_server {
        () => {{
            use rouille::Response;
            use rouille::Server;

            let server = Server::new("localhost:0", |request| match request.url().as_str() {
                "/update" => Response::from_data(
                    "text/html",
                    include_bytes!("../assets/namecheap-update.xml").to_vec(),
                ),
                _ => Response::empty_404(),
            })
            .unwrap();

            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            let addr = server.server_addr().clone();
            std::thread::spawn(move || {
                while let Err(_) = rx.try_recv() {
                    server.poll();
                    std::thread::sleep(std::time::Duration::from_millis(50))
                }
            });
            (tx, addr)
        }};
    }

    #[tokio::test]
    async fn test_namecheap_update() {
        let (tx, addr) = namecheap_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = NamecheapConfig {
            base_url: format!("http://{}", addr),
            domain: String::from("example.com"),
            ddns_password: String::from("secret-1"),
            records: vec![String::from("@")],
        };

        let summary = update_domains(&http_client, &config, new_ip).await.unwrap();
        tx.send(()).unwrap();

        assert_eq!(
            summary,
            Updates {
                current: 0,
                updated: 1,
                missing: 0,
            }
        );
    }
}
