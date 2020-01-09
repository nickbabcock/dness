use crate::config::CloudflareConfig;
use crate::dns::Updates;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error;
use std::fmt;
use std::net::Ipv4Addr;

#[derive(Serialize, PartialEq, Clone, Debug)]
struct CloudflareUpdate<'a> {
    type_: &'static str,
    name: &'a str,
    content: &'a str,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
struct CloudflareZone {
    id: String,
    name: String,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
struct CloudflareDnsRecord {
    id: String,
    name: String,
    content: String,
}

#[derive(Serialize, PartialEq, Clone, Debug)]
struct CloudflareDnsRecordUpdate {
    #[serde(rename = "type")]
    type_: &'static str,
    name: String,
    content: String,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
pub struct CloudflareError {
    code: i32,
    message: String,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
struct CloudflareResponse<T> {
    result: Option<T>,
    result_info: Option<CloudflareResultInfo>,
    success: bool,
    errors: Vec<CloudflareError>,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
struct CloudflareResultInfo {
    page: i32,
    per_page: i32,
    total_pages: i32,
    count: i32,
    total_count: i32,
}

#[derive(Clone, Debug)]
struct CloudflareClient<'a> {
    email: String,
    key: String,
    zone_name: String,
    zone_id: String,
    records: HashSet<String>,
    client: &'a reqwest::Client,
}

#[derive(Debug)]
pub struct ClError {
    kind: ClErrorKind,
}

#[derive(Debug)]
pub enum ClErrorKind {
    SendHttp(&'static str, reqwest::Error),
    DecodeHttp(&'static str, reqwest::Error),
    ErrorResponse(&'static str, Vec<CloudflareError>),
    MissingResult(&'static str),
    UnexpectedNumberOfZones(usize),
}

impl error::Error for ClError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            ClErrorKind::SendHttp(_, ref e) => Some(e),
            ClErrorKind::DecodeHttp(_, ref e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for ClError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "communicating with cloudflare: ")?;
        match self.kind {
            ClErrorKind::SendHttp(action, ref _e) => write!(f, "http send error for {}", action),
            ClErrorKind::DecodeHttp(action, ref _e) => {
                write!(f, "decoding response for {}", action)
            }
            ClErrorKind::ErrorResponse(action, ref errors) => {
                write!(f, "cloudflare returned an error response for {}: ", action)?;
                for error in errors {
                    write!(f, "{}: {}. ", error.code, error.message)?;
                }
                Ok(())
            }
            ClErrorKind::MissingResult(action) => {
                write!(f, "no cloudflare result found for {}", action)
            }
            ClErrorKind::UnexpectedNumberOfZones(zones) => {
                write!(f, "expected 1 zone to be returned, not {}", zones)
            }
        }
    }
}

impl<'a> CloudflareClient<'a> {
    async fn create<'b>(
        client: &'b reqwest::Client,
        config: &CloudflareConfig,
    ) -> Result<CloudflareClient<'b>, ClError> {
        // Need to translate our zone name into an id
        let response: CloudflareResponse<Vec<CloudflareZone>> = client
            .get("https://api.cloudflare.com/client/v4/zones")
            .query(&[("name", &config.zone)])
            .header("X-Auth-Email", config.email.clone())
            .header("X-Auth-Key", config.key.clone())
            .send()
            .await
            .map_err(|e| ClError {
                kind: ClErrorKind::SendHttp("get zones", e),
            })?
            .json()
            .await
            .map_err(|e| ClError {
                kind: ClErrorKind::DecodeHttp("get zones", e),
            })?;

        if !response.success {
            Err(ClError {
                kind: ClErrorKind::ErrorResponse("zones", response.errors.clone()),
            })
        } else if let Some(zone) = response.result {
            if zone.len() != 1 {
                return Err(ClError {
                    kind: ClErrorKind::UnexpectedNumberOfZones(zone.len()),
                });
            }

            let zone_id = zone[0].id.clone();

            Ok(CloudflareClient {
                email: config.email.clone(),
                key: config.key.clone(),
                zone_name: config.zone.clone(),
                zone_id,
                records: config.records.iter().cloned().collect(),
                client,
            })
        } else {
            Err(ClError {
                kind: ClErrorKind::MissingResult("zones"),
            })
        }
    }

    // Grab all the sub domains in the zone, but since there can be many of them, cloudflare
    // paginates the results.
    async fn paginate_domains(&self) -> Result<Vec<CloudflareDnsRecord>, ClError> {
        let mut done = false;
        let mut page = 0;
        let mut dns_records: Vec<CloudflareDnsRecord> = Vec::new();

        let record_url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.zone_id
        );

        while !done {
            page += 1;

            debug!("grabbing page {} from {}", page, record_url);
            let response: CloudflareResponse<Vec<CloudflareDnsRecord>> = self
                .client
                .get(&record_url)
                .query(&[("page", page)])
                .query(&[("type", "A")])
                .header("X-Auth-Email", self.email.clone())
                .header("X-Auth-Key", self.key.clone())
                .send()
                .await
                .map_err(|e| ClError {
                    kind: ClErrorKind::SendHttp("get records", e),
                })?
                .json()
                .await
                .map_err(|e| ClError {
                    kind: ClErrorKind::DecodeHttp("get records", e),
                })?;

            if !response.success {
                return Err(ClError {
                    kind: ClErrorKind::ErrorResponse("get records", response.errors.clone()),
                });
            } else if let Some(records) = response.result {
                dns_records.extend(records);

                if let Some(info) = response.result_info {
                    done = info.total_pages <= page;
                } else {
                    done = true;
                    warn!(
                        "did not receive a result info page for {}, assuming no more results",
                        self.zone_name
                    );
                }
            } else {
                return Err(ClError {
                    kind: ClErrorKind::MissingResult("get records"),
                });
            }
        }

        Ok(dns_records)
    }

    // Logs the domains found in the config but not in cloudflare
    fn log_missing_domains(&self, remote_domains: &[CloudflareDnsRecord]) -> usize {
        let actual = remote_domains
            .iter()
            .map(|x| &x.name)
            .cloned()
            .collect::<HashSet<String>>();
        crate::core::log_missing_domains(&self.records, &actual, "cloudflare", &self.zone_name)
    }

    async fn update(&self, addr: Ipv4Addr) -> Result<Updates, ClError> {
        let mut dns_records = self.paginate_domains().await?;
        let missing = self.log_missing_domains(&dns_records) as i32;
        let mut current = 0;
        let mut updated = 0;

        let recs = dns_records
            .iter_mut()
            .filter(|x| self.records.contains(&x.name));

        for record in recs {
            match record.content.parse::<Ipv4Addr>() {
                Ok(ip) => {
                    if ip != addr {
                        updated += 1;
                        self.update_record(&record, addr).await?;

                        info!(
                            "{} from zone {} updated from {} to {}",
                            record.name, self.zone_name, record.content, addr
                        )
                    } else {
                        current += 1;
                        debug!(
                            "{} from zone {} is already current",
                            record.name, self.zone_name
                        )
                    }
                }
                Err(ref e) => {
                    updated += 1;
                    warn!("could not parse domain {} address {} as ipv4 -- will replace it. Original error: {}", record.name, record.content, e);
                    self.update_record(&record, addr).await?;

                    info!(
                        "{} from zone {} update from {} to {}",
                        record.name, self.zone_name, record.content, addr
                    )
                }
            }
        }

        Ok(Updates {
            updated,
            current,
            missing,
        })
    }

    async fn update_record(&self, record: &CloudflareDnsRecord, addr: Ipv4Addr) -> Result<(), ClError> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.zone_id, record.id
        );

        let update = CloudflareDnsRecordUpdate {
            name: record.name.clone(),
            type_: "A",
            content: addr.to_string(),
        };

        let response: CloudflareResponse<CloudflareDnsRecord> = self
            .client
            .put(&url)
            .header("X-Auth-Email", self.email.clone())
            .header("X-Auth-Key", self.key.clone())
            .json(&update)
            .send()
            .await
            .map_err(|e| ClError {
                kind: ClErrorKind::SendHttp("update dns", e),
            })?
            .json()
            .await
            .map_err(|e| ClError {
                kind: ClErrorKind::DecodeHttp("update dns", e),
            })?;

        if !response.success {
            Err(ClError {
                kind: ClErrorKind::ErrorResponse("update dns", response.errors),
            })
        } else {
            Ok(())
        }
    }
}

/// Updating cloudflare domain works as follows:
///  1. Send GET to translate the zone (example.com) to cloudflare's id
///  2. Send GET to find all the domains under the zone and their ids
///    - Cloudflare paginates the response to handle many subdomains
///    - It is possible to query for individual domains but as long as more
///      than one desired domain in each page -- this methods cuts down requests
///  3. Each desired domain in the config is checked to ensure that it is set to our address. In
///     this way cloudflare is our cache (to guard against nefarious users updating out of band)
pub async fn update_domains(
    client: &reqwest::Client,
    config: &CloudflareConfig,
    addr: Ipv4Addr,
) -> Result<Updates, ClError> {
    CloudflareClient::create(&client, &config).await?.update(addr).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_cloudflare_error() {
        let json_str = &include_str!("../assets/cloudflare-error.json");
        let response: CloudflareResponse<String> = serde_json::from_str(json_str).unwrap();
        assert_eq!(
            response,
            CloudflareResponse {
                result: None,
                result_info: None,
                success: false,
                errors: vec![CloudflareError {
                    code: 1003,
                    message: String::from("Invalid or missing zone id."),
                }]
            }
        );
    }

    #[test]
    fn deserialize_cloudflare_zone() {
        let json_str = &include_str!("../assets/cloudflare-zone-response.json");
        let response: CloudflareResponse<Vec<CloudflareZone>> =
            serde_json::from_str(json_str).unwrap();

        assert_eq!(
            response,
            CloudflareResponse {
                result: Some(vec![CloudflareZone {
                    id: String::from("aaaabbbb"),
                    name: String::from("example.com"),
                }]),
                result_info: Some(CloudflareResultInfo {
                    page: 1,
                    per_page: 20,
                    total_pages: 1,
                    count: 1,
                    total_count: 1,
                }),
                success: true,
                errors: vec![]
            }
        );
    }

    #[test]
    fn deserialize_cloudflare_update_response() {
        let json_str = &include_str!("../assets/cloudflare-update-response.json");
        let response: CloudflareResponse<CloudflareDnsRecord> =
            serde_json::from_str(json_str).unwrap();

        assert_eq!(
            response,
            CloudflareResponse {
                result: Some(CloudflareDnsRecord {
                    id: String::from("372e67954025e0ba6aaa6d586b9e0b59"),
                    name: String::from("example.com"),
                    content: String::from("198.51.100.4"),
                }),
                result_info: None,
                success: true,
                errors: vec![]
            }
        );
    }
}
