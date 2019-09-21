use crate::config::GoDaddyConfig;
use crate::dns::Updates;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap as Map;
use std::collections::HashSet;
use std::error;
use std::fmt;
use std::net::Ipv4Addr;

#[derive(Debug)]
pub struct DnessError {
    kind: DnessErrorKind,
}

impl DnessError {
    fn send_http(url: &str, context: &str, source: reqwest::Error) -> DnessError {
        DnessError {
            kind: DnessErrorKind::SendHttp {
                url: String::from(url),
                context: String::from(context),
                source,
            },
        }
    }

    fn bad_response(url: &str, context: &str, source: reqwest::Error) -> DnessError {
        DnessError {
            kind: DnessErrorKind::BadResponse {
                url: String::from(url),
                context: String::from(context),
                source,
            },
        }
    }

    fn deserialize(url: &str, context: &str, source: reqwest::Error) -> DnessError {
        DnessError {
            kind: DnessErrorKind::Deserialize {
                url: String::from(url),
                context: String::from(context),
                source,
            },
        }
    }
}

impl error::Error for DnessError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            DnessErrorKind::SendHttp { ref source, .. } => Some(source),
            DnessErrorKind::BadResponse { ref source, .. } => Some(source),
            DnessErrorKind::Deserialize { ref source, .. } => Some(source),
        }
    }
}

impl fmt::Display for DnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DnessErrorKind::SendHttp { url, context, .. } => write!(
                f,
                "unable to send http request for {}: url attempted: {}",
                context, url
            ),
            DnessErrorKind::BadResponse { url, context, .. } => write!(
                f,
                "received bad http response for {}: url attempted: {}",
                context, url
            ),
            DnessErrorKind::Deserialize { url, context, .. } => write!(
                f,
                "unable to deserialize response for {}: url attempted: {}",
                context, url
            ),
        }
    }
}

#[derive(Debug)]
pub enum DnessErrorKind {
    SendHttp {
        url: String,
        context: String,
        source: reqwest::Error,
    },
    BadResponse {
        url: String,
        context: String,
        source: reqwest::Error,
    },
    Deserialize {
        url: String,
        context: String,
        source: reqwest::Error,
    },
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
struct GoRecord {
    data: String,
    name: String,

    #[serde(flatten)]
    other: Map<String, Value>,
}

#[derive(Clone, Debug)]
struct GoClient<'a> {
    base_url: String,
    domain: String,
    key: String,
    secret: String,
    records: HashSet<String>,
    client: &'a reqwest::Client,
}

impl<'a> GoClient<'a> {
    fn log_missing_domains(&self, remote_domains: &[GoRecord]) -> usize {
        let actual = remote_domains
            .iter()
            .map(|x| &x.name)
            .cloned()
            .collect::<HashSet<String>>();
        crate::core::log_missing_domains(&self.records, &actual, "GoDaddy", &self.domain)
    }

    fn auth_header(&self) -> String {
        format!("sso-key {}:{}", self.key, self.secret)
    }

    fn fetch_records(&self) -> Result<Vec<GoRecord>, DnessError> {
        let get_url = format!("{}/v1/domains/{}/records/A", self.base_url, self.domain);
        Ok(self
            .client
            .get(&get_url)
            .header("Authorization", self.auth_header())
            .send()
            .map_err(|e| DnessError::send_http(&get_url, "godaddy fetch records", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&get_url, "godaddy fetch records", e))?
            .json()
            .map_err(|e| DnessError::deserialize(&get_url, "godaddy fetch records", e))?)
    }

    fn update_record(&self, record: &GoRecord, addr: Ipv4Addr) -> Result<(), DnessError> {
        let put_url = format!(
            "{}/v1/domains/{}/records/A/{}",
            self.base_url, self.domain, record.name
        );

        self.client
            .put(&put_url)
            .header("Authorization", self.auth_header())
            .json(&vec![GoRecord {
                data: addr.to_string(),
                ..record.clone()
            }])
            .send()
            .map_err(|e| DnessError::send_http(&put_url, "godaddy update records", e))?
            .error_for_status()
            .map_err(|e| DnessError::bad_response(&put_url, "godaddy update records", e))?;

        Ok(())
    }

    fn ensure_current_ip(&self, record: &GoRecord, addr: Ipv4Addr) -> Result<Updates, DnessError> {
        let mut current = 0;
        let mut updated = 0;
        match record.data.parse::<Ipv4Addr>() {
            Ok(ip) => {
                if ip != addr {
                    updated += 1;
                    self.update_record(&record, addr)?;

                    info!(
                        "{} from domain {} updated from {} to {}",
                        record.name, self.domain, record.data, addr
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
                warn!("could not parse domain {} address {} as ipv4 -- will replace it. Original error: {}", record.name, record.data, e);
                self.update_record(&record, addr)?;

                info!(
                    "{} from domain {} updated from {} to {}",
                    record.name, self.domain, record.data, addr
                )
            }
        }

        Ok(Updates {
            current,
            updated,
            ..Updates::default()
        })
    }
}

/// GoDaddy dynamic dns service works as the following:
///
/// 1. Send a GET request to find all records in the domain
/// 2. Find all the expected records (and log those that are missing) and check their current IP
/// 3. Update the remote IP as needed, ensuring that original properties are preserved in the
///    upload, so that we don't overwrite a property like TTL.
pub fn update_domains(
    client: &reqwest::Client,
    config: &GoDaddyConfig,
    addr: Ipv4Addr,
) -> Result<Updates, DnessError> {
    let go_client = GoClient {
        base_url: config.base_url.trim_end_matches('/').to_string(),
        domain: config.domain.clone(),
        key: config.key.clone(),
        secret: config.secret.clone(),
        records: config.records.iter().cloned().collect(),
        client,
    };

    let records = go_client.fetch_records()?;
    let missing = go_client.log_missing_domains(&records) as i32;
    let mut summary = Updates {
        missing,
        ..Updates::default()
    };

    for record in records {
        if go_client.records.contains(&record.name) {
            summary += go_client.ensure_current_ip(&record, addr)?;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_http::HttpService;
    use actix_http_test::{TestServer, TestServerRuntime};
    use actix_web::{http::StatusCode, web, App, HttpResponse};
    use serde_json::json;

    #[test]
    fn deserialize_go_records() {
        let json_str = &include_str!("../assets/godaddy-get-records.json");
        let response: Vec<GoRecord> = serde_json::from_str(json_str).unwrap();
        let mut expected = Map::new();
        expected.insert(String::from("ttl"), Value::Number(600.into()));
        expected.insert(String::from("type"), Value::String(String::from("A")));
        assert_eq!(
            response,
            vec![GoRecord {
                name: String::from("@"),
                data: String::from("256.256.256.256"),
                other: expected,
            }]
        );
    }

    #[test]
    fn serialize_go_records() {
        let mut other = Map::new();
        other.insert(String::from("ttl"), Value::Number(600.into()));
        let rec = GoRecord {
            data: String::from("256.256.256.256"),
            name: String::from("@"),
            other,
        };

        let actual = serde_json::to_string(&rec).unwrap();
        let expected = serde_json::to_string(&json!({
            "name": "@",
            "data": "256.256.256.256",
            "ttl": 600
        }))
        .unwrap();
        assert_eq!(actual, expected);
    }

    fn unparseable_ipv4() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(include_str!("../assets/godaddy-get-records.json"))
    }

    fn grabbag_site() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(r#"[{"name": "@", "data": "2.2.2.2"}, {"name": "a", "data": "2.1.2.2"}]"#)
    }

    fn update() -> HttpResponse {
        HttpResponse::new(StatusCode::OK)
    }

    fn create_test_server() -> TestServerRuntime {
        TestServer::new(|| {
            HttpService::new(
                App::new()
                    .route(
                        "/v1/domains/domain-1.com/records/A",
                        web::to(unparseable_ipv4),
                    )
                    .route("/v1/domains/domain-1.com/records/A/@", web::to(update))
                    .route("/v1/domains/domain-2.com/records/A", web::to(grabbag_site))
                    .route("/v1/domains/domain-2.com/records/A/@", web::to(update))
                    .route("/v1/domains/domain-2.com/records/A/a", web::to(update)),
            )
        })
    }

    fn test_config(server: &TestServerRuntime) -> GoDaddyConfig {
        GoDaddyConfig {
            base_url: String::from(server.url("")),
            domain: String::from("domain-1.com"),
            key: String::from("key-1"),
            secret: String::from("secret-1"),
            records: vec![String::from("@")],
        }
    }

    #[test]
    fn test_godaddy_unparseable_ipv4() {
        let server = create_test_server();
        let http_client = reqwest::Client::new();
        let new_ip = Ipv4Addr::new(2, 2, 2, 2);
        let config = test_config(&server);
        let summary = update_domains(&http_client, &config, new_ip).unwrap();
        assert_eq!(
            summary,
            Updates {
                current: 0,
                updated: 1,
                missing: 0,
            }
        );
    }

    #[test]
    fn test_godaddy_grabbag() {
        let server = create_test_server();
        let http_client = reqwest::Client::new();
        let new_ip = Ipv4Addr::new(2, 2, 2, 2);
        let config = GoDaddyConfig {
            domain: String::from("domain-2.com"),
            records: vec![String::from("@"), String::from("a"), String::from("b")],
            ..test_config(&server).clone()
        };
        let summary = update_domains(&http_client, &config, new_ip).unwrap();
        assert_eq!(
            summary,
            Updates {
                current: 1,
                updated: 1,
                missing: 1,
            }
        );
    }
}
