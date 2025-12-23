use crate::{config::NoIpConfig, core::Updates, dns::DnsResolver, errors::DnessError};
use log::{info, warn};
use std::net::IpAddr;

#[derive(Debug)]
pub struct NoIpProvider<'a> {
    client: &'a reqwest::Client,
    config: &'a NoIpConfig,
}

impl NoIpProvider<'_> {
    /// https://www.noip.com/integrate/request
    pub async fn update_domain(&self, wan: IpAddr) -> Result<(), DnessError> {
        let base = self.config.base_url.trim_end_matches('/').to_string();
        let get_url = format!("{}/nic/update", base);
        let response = self
            .client
            .get(&get_url)
            .query(&[
                ("hostname", &self.config.hostname),
                ("myip", &wan.to_string()),
            ])
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .map_err(|e| DnessError::send_http(&get_url, "noip update", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&get_url, "noip update", e))?
            .text()
            .await
            .map_err(|e| DnessError::deserialize(&get_url, "noip update", e))?;

        if !response.contains("good") {
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
    config: &NoIpConfig,
    wan: IpAddr,
) -> Result<Updates, DnessError> {
    let resolver = DnsResolver::create_cloudflare().await?;
    let dns_query = format!("{}.", &config.hostname);
    let response = resolver.ip_lookup(&dns_query, wan.into()).await;
    let provider = NoIpProvider { client, config };
    match response {
        Ok(ip) => {
            if ip == wan {
                Ok(Updates {
                    current: 1,
                    ..Updates::default()
                })
            } else {
                provider.update_domain(wan).await?;
                info!("{} updated from {} to {}", config.hostname, ip, wan);
                Ok(Updates {
                    updated: 1,
                    ..Updates::default()
                })
            }
        }
        Err(e) => {
            // Could be a network issue or it could be that the record didn't exist.
            warn!(
                "resolving noip ({}) encountered an error: {}",
                config.hostname, e
            );
            Ok(Updates {
                missing: 1,
                ..Updates::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IpType;
    use std::net::Ipv4Addr;

    macro_rules! noip_server {
        () => {{
            use rouille::Response;
            use rouille::Server;

            let server = Server::new("localhost:0", |request| match request.url().as_str() {
                "/nic/update" => Response::from_data("text/plain", b"good 2.2.2.2".to_vec()),
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
    async fn test_noip_update() {
        let (tx, addr) = noip_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = NoIpConfig {
            base_url: format!("http://{}", addr),
            hostname: String::from("example.com"),
            username: String::from("me@example.com"),
            password: String::from("my-pass"),
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
