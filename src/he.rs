use crate::config::HeConfig;
use crate::core::Updates;
use crate::dns::DnsResolver;
use crate::errors::DnessError;
use log::{info, warn};
use std::net::IpAddr;

#[derive(Debug)]
pub struct HeProvider<'a> {
    config: &'a HeConfig,
}

impl HeProvider<'_> {
    /// https://dns.he.net/docs.html
    pub async fn update_domain(&self, host: &str, wan: IpAddr) -> Result<(), DnessError> {
        let base = self.config.base_url.trim_end_matches('/').to_string();
        let url = format!("{}/nic/update", base);
        let params = [
            ("hostname", host),
            ("password", &self.config.password),
            ("myip", &wan.to_string()),
        ];

        // annoyingly it looks like he closes the connection on every update
        // so we have to allocate a new client for every request
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| DnessError::send_http(&url, "he update", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&url, "he update", e))?
            .text()
            .await
            .map_err(|e| DnessError::deserialize(&url, "he update", e))?;

        if !response.contains("good") && !response.contains("nochg") {
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
    _client: &reqwest::Client,
    config: &HeConfig,
    wan: IpAddr,
) -> Result<Updates, DnessError> {
    // uses the same strategy as namecheap where we get the current records
    // via dns and check if they need to be updated
    let resolver = DnsResolver::create_cloudflare().await?;
    let he = HeProvider { config };

    let mut results = Updates::default();

    for record in &config.records {
        let host_record = if record == "@" {
            config.hostname.clone()
        } else {
            format!("{}.{}", record, &config.hostname)
        };

        let dns_query = format!("{}.", &host_record);
        let response = resolver.ip_lookup(&dns_query, wan.into()).await;

        match response {
            Ok(ip) => {
                if ip == wan {
                    results.current += 1;
                } else {
                    he.update_domain(&host_record, wan).await?;
                    info!(
                        "{} from domain {} updated from {} to {}",
                        record, config.hostname, ip, wan
                    );
                    results.updated += 1;
                }
            }
            Err(e) => {
                // Could be a network issue or it could be that the record didn't exist.
                warn!(
                    "resolving he record ({}) encountered an error: {}",
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
    use crate::config::IpType;
    use std::net::Ipv4Addr;

    macro_rules! he_server {
        () => {{
            use rouille::Response;
            use rouille::Server;

            let server = Server::new("localhost:0", |request| match request.url().as_str() {
                "/nic/update" => Response::from_data("text/html", (b"good 2.2.2.2").to_vec()),
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
    async fn test_he_update() {
        let (tx, addr) = he_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = HeConfig {
            base_url: format!("http://{}", addr),
            hostname: String::from("example.com"),
            password: String::from("secret-1"),
            records: vec![String::from("@")],
            ip_types: vec![IpType::V4],
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
