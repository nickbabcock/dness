use crate::config::IpType;
use crate::config::PorkbunConfig;
use crate::core::Updates;
use crate::errors::DnessError;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap as Map;
use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
struct PorkbunResponse {
    status: String,
    cloudflare: String,
    records: Vec<PorkbunRecord>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
struct PorkbunRecord {
    id: String,
    name: String,
    r#type: String,
    content: String,
    ttl: String,
    prio: Option<String>,

    #[serde(flatten)]
    other: Map<String, Value>,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
struct PorkbunRecordsEditRequest {
    apikey: String,
    secretapikey: String,
    name: String,
    r#type: String,
    content: String,
    ttl: String,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
struct PorkbunRecordsRequest {
    apikey: String,
    secretapikey: String,
}

#[derive(Clone, Debug)]
struct PorkbunClient<'a> {
    base_url: String,
    domain: String,
    key: String,
    secret: String,
    records: HashSet<String>,
    client: &'a reqwest::Client,
}

impl PorkbunClient<'_> {
    fn strip_domain_from_name(&self, name: &str) -> String {
        name.trim_end_matches(&self.domain)
            .trim_end_matches('.')
            .into()
    }

    fn log_missing_domains(&self, remote_domains: &[PorkbunRecord]) -> usize {
        let actual = remote_domains
            .iter()
            .map(|x| self.strip_domain_from_name(&x.name))
            .collect::<HashSet<String>>();
        crate::core::log_missing_domains(&self.records, &actual, "Porkbun", &self.domain)
    }

    async fn fetch_records(&self, ip_type: IpType) -> Result<Vec<PorkbunRecord>, DnessError> {
        let post_url = format!("{}/dns/retrieve/{}", self.base_url, self.domain);
        let response = self
            .client
            .post(post_url.clone())
            .json(&PorkbunRecordsRequest {
                apikey: self.key.clone(),
                secretapikey: self.secret.clone(),
            })
            .send()
            .await
            .map_err(|e| DnessError::send_http(&post_url, "porkbun fetch records", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&post_url, "porkbun fetch records", e))?
            .json::<PorkbunResponse>()
            .await
            .map_err(|e| DnessError::deserialize(&post_url, "porkbun fetch records", e))?
            .records
            .into_iter()
            .filter(|r| r.r#type == ip_type.record_type())
            .collect();
        Ok(response)
    }

    async fn update_record(&self, record: &PorkbunRecord, addr: IpAddr) -> Result<(), DnessError> {
        let post_url = format!("{}/dns/edit/{}/{}", self.base_url, self.domain, record.id);

        self.client
            .post(&post_url)
            .json(&PorkbunRecordsEditRequest {
                apikey: self.key.clone(),
                secretapikey: self.secret.clone(),
                name: self.strip_domain_from_name(&record.name),
                content: addr.to_string(),
                ttl: record.ttl.clone(),
                r#type: record.r#type.clone(),
            })
            .send()
            .await
            .map_err(|e| DnessError::send_http(&post_url, "porkbun update records", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&post_url, "porkbun update records", e))?;

        Ok(())
    }

    async fn ensure_current_ip(
        &self,
        record: &PorkbunRecord,
        addr: IpAddr,
    ) -> Result<Updates, DnessError> {
        let mut current = 0;
        let mut updated = 0;
        match record.content.parse::<IpAddr>() {
            Ok(ip) => {
                if ip != addr {
                    updated += 1;
                    self.update_record(record, addr).await?;

                    info!(
                        "{} from domain {} updated from {} to {}",
                        record.name, self.domain, record.content, addr
                    )
                } else {
                    current += 1;
                    debug!(
                        "{} from domain {} is already current",
                        record.name, self.domain
                    )
                }
            }
            Err(ref e) => {
                updated += 1;
                warn!("could not parse domain {} address {} as ipv4 -- will replace it. Original error: {}", record.name, record.content, e);
                self.update_record(record, addr).await?;

                info!(
                    "{} from domain {} updated from {} to {}",
                    record.name, self.domain, record.content, addr
                )
            }
        }

        Ok(Updates {
            updated,
            current,
            ..Updates::default()
        })
    }
}

/// Porkbun dynamic dns service works as the following:
///
/// 1. Send a GET request to find all records in the domain
/// 2. Filter records to just records in VALID_RECORD_TYPES, only "A" records when written
/// 3. Find all the expected records (and log those that are missing) and check their current IP
/// 4. Update the remote IP as needed, ensuring that original properties are preserved in the
///    upload, so that we don't overwrite a property like TTL.
pub async fn update_domains(
    client: &reqwest::Client,
    config: &PorkbunConfig,
    addr: IpAddr,
) -> Result<Updates, DnessError> {
    let porkbun_client = PorkbunClient {
        base_url: config.base_url.trim_end_matches('/').to_string(),
        domain: config.domain.clone(),
        key: config.key.clone(),
        secret: config.secret.clone(),
        records: config
            .records
            .iter()
            .map(|r| {
                // To be consistent with other dns providers we allow the user to use '@' for root
                // domain. Porkbun uses an empty string, so we map that here.
                if r == "@" {
                    String::from("")
                } else {
                    r.to_string()
                }
            })
            .collect(),
        client,
    };

    let records = porkbun_client.fetch_records(addr.into()).await?;
    let missing = porkbun_client.log_missing_domains(&records) as i32;
    let mut summary = Updates {
        missing,
        ..Updates::default()
    };

    for record in records {
        if porkbun_client
            .records
            .contains(&porkbun_client.strip_domain_from_name(&record.name))
        {
            summary += porkbun_client.ensure_current_ip(&record, addr).await?;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IpType;
    use std::net::Ipv4Addr;

    #[test]
    fn deserialize_porkbun_response() {
        let json_str = &include_str!("../assets/porkbun-get-records.json");
        let response: PorkbunResponse = serde_json::from_str(json_str).unwrap();
        let mut expected_1 = Map::new();
        expected_1.insert(String::from("notes"), Value::String(String::from("")));
        let mut expected_2 = Map::new();
        expected_2.insert(String::from("notes"), Value::Null);
        assert_eq!(
            response,
            PorkbunResponse {
                status: String::from("SUCCESS"),
                cloudflare: String::from("enabled"),
                records: vec![
                    PorkbunRecord {
                        id: String::from("356408594"),
                        name: String::from("sub.example.com"),
                        r#type: String::from("A"),
                        content: String::from("2.2.2.2"),
                        ttl: String::from("600"),
                        prio: Some(String::from("0")),
                        other: expected_1,
                    },
                    PorkbunRecord {
                        id: String::from("354399918"),
                        name: String::from("example.com"),
                        r#type: String::from("A"),
                        content: String::from("2.2.2.2"),
                        ttl: String::from("700"),
                        prio: Some(String::from("0")),
                        other: expected_2.clone(),
                    },
                    PorkbunRecord {
                        id: String::from("354379285"),
                        name: String::from("example.com"),
                        r#type: String::from("NS"),
                        content: String::from("maceio.porkbun.com"),
                        ttl: String::from("86400"),
                        prio: None,
                        other: expected_2.clone(),
                    }
                ]
            }
        );
    }

    macro_rules! porkbun_rouille_server {
        () => {{
            use rouille::Response;
            use rouille::Server;

            let server = Server::new("localhost:0", |request| match request.url().as_str() {
                "/api/json/v3/dns/retrieve/example.com" => Response::from_data(
                    "application/json",
                    include_bytes!("../assets/porkbun-get-records.json").to_vec(),
                ),
                "/api/json/v3/dns/edit/example.com/356408594" => {
                    Response::from_data("application/json", r#"{"status": "SUCCESS"}"#)
                }
                "/api/json/v3/dns/edit/example.com/354399918" => {
                    Response::from_data("application/json", r#"{"status": "SUCCESS"}"#)
                }
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
    async fn test_porkbun_update() {
        let (tx, addr) = porkbun_rouille_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 1));
        let config = PorkbunConfig {
            base_url: format!("http://{}/api/json/v3", addr),
            domain: String::from("example.com"),
            key: String::from("key-1"),
            secret: String::from("secret-1"),
            records: vec![String::from("@"), String::from("sub")],
            ip_types: vec![IpType::V4],
        };

        let summary = update_domains(&http_client, &config, new_ip).await.unwrap();
        tx.send(()).unwrap();

        assert_eq!(
            summary,
            Updates {
                current: 0,
                updated: 2,
                missing: 0,
            }
        )
    }

    #[tokio::test]
    async fn test_porkbun_current() {
        let (tx, addr) = porkbun_rouille_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = PorkbunConfig {
            base_url: format!("http://{}/api/json/v3", addr),
            domain: String::from("example.com"),
            key: String::from("key-1"),
            secret: String::from("secret-1"),
            records: vec![String::from("@"), String::from("sub")],
            ip_types: vec![IpType::V4],
        };

        let summary = update_domains(&http_client, &config, new_ip).await.unwrap();
        tx.send(()).unwrap();

        assert_eq!(
            summary,
            Updates {
                current: 2,
                updated: 0,
                missing: 0,
            }
        )
    }

    #[tokio::test]
    async fn test_porkbun_missing() {
        let (tx, addr) = porkbun_rouille_server!();
        let http_client = reqwest::Client::new();
        let new_ip = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let config = PorkbunConfig {
            base_url: format!("http://{}/api/json/v3", addr),
            domain: String::from("example.com"),
            key: String::from("key-1"),
            secret: String::from("secret-1"),
            records: vec![String::from("@"), String::from("sub"), String::from("sub2")],
            ip_types: vec![IpType::V4],
        };

        let summary = update_domains(&http_client, &config, new_ip).await.unwrap();
        tx.send(()).unwrap();

        assert_eq!(
            summary,
            Updates {
                current: 2,
                updated: 0,
                missing: 1,
            }
        )
    }
}
