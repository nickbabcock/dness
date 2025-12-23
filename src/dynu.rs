use crate::config::DynuConfig;
use crate::core::Updates;
use crate::dns::DnsResolver;
use crate::errors::DnessError;
use log::{info, warn};
use std::net::IpAddr;

#[derive(Debug)]
pub struct DynuProvider<'a> {
    client: &'a reqwest::Client,
    config: &'a DynuConfig,
}

impl DynuProvider<'_> {
    pub async fn update_domain(&self, host: &str, wan: IpAddr) -> Result<(), DnessError> {
        let base = self.config.base_url.trim_end_matches('/').to_string();
        let get_url = format!("{}/nic/update", base);
        let mut params = vec![("hostname", self.config.hostname.clone())];

        match wan {
            IpAddr::V4(ipv4_addr) => {
                params.push(("myip", ipv4_addr.to_string()));
                params.push(("myipv6", "no".to_owned()))
            }
            IpAddr::V6(ipv6_addr) => {
                params.push(("myip", "no".to_owned()));
                params.push(("myipv6", ipv6_addr.to_string()))
            }
        }

        if host != "@" {
            params.push(("alias", String::from(host)));
        }

        let response = self
            .client
            .get(&get_url)
            .query(&params)
            .basic_auth(
                self.config.username.clone(),
                Some(self.config.password.clone()),
            )
            .send()
            .await
            .map_err(|e| DnessError::send_http(&get_url, "dynu update", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&get_url, "dynu update", e))?
            .text()
            .await
            .map_err(|e| DnessError::deserialize(&get_url, "dynu update", e))?;

        if !response.contains("nochg") && !response.contains("good") {
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
    config: &DynuConfig,
    wan: IpAddr,
) -> Result<Updates, DnessError> {
    let resolver = DnsResolver::create_cloudflare().await?;
    let dynu_provider = DynuProvider { client, config };

    let mut results = Updates::default();

    for record in &config.records {
        let dns_query = if record == "@" {
            format!("{}.", config.hostname)
        } else {
            format!("{}.{}.", record, config.hostname)
        };

        let response = resolver.ip_lookup(&dns_query, wan.into()).await;

        match response {
            Ok(ip) => {
                if ip == wan {
                    results.current += 1;
                } else {
                    dynu_provider.update_domain(record, wan).await?;
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
                    "resolving dynu record ({}) encountered an error: {}",
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

    macro_rules! dynu_server {
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
    async fn test_dynu_update() {
        let (tx, addr) = dynu_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = DynuConfig {
            base_url: format!("http://{}", addr),
            hostname: String::from("example.com"),
            username: String::from("myusername"),
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
