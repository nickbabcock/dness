use handlebars::{Handlebars, RenderError, TemplateError};
use log::LevelFilter;
use serde::Deserialize;
use std::fmt;
use std::fs::File;
use std::io::Error as IoError;
use std::io::Read;
use std::path::Path;
use std::{collections::HashMap, error};

#[derive(Debug)]
pub struct ConfigError {
    kind: ConfigErrorKind,
}

#[derive(Debug)]
pub enum ConfigErrorKind {
    FileNotFound(IoError),
    Misread(IoError),
    Parse(toml::de::Error),
    Template(TemplateError),
    Render(RenderError),
}

impl error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.kind {
            ConfigErrorKind::FileNotFound(ref e) => Some(e),
            ConfigErrorKind::Misread(ref e) => Some(e),
            ConfigErrorKind::Parse(ref e) => Some(e),
            ConfigErrorKind::Template(ref e) => Some(e),
            ConfigErrorKind::Render(ref e) => Some(e),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "config issue: ")?;
        match self.kind {
            ConfigErrorKind::FileNotFound(ref _e) => write!(f, "file not found"),
            ConfigErrorKind::Misread(ref _e) => write!(f, "unable to read file"),
            ConfigErrorKind::Parse(ref _e) => write!(f, "a parsing error"),
            ConfigErrorKind::Template(ref _e) => write!(f, "config template error"),
            ConfigErrorKind::Render(ref _e) => write!(f, "config template rendering error"),
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct DnsConfig {
    #[serde(default = "default_resolver")]
    pub ip_resolver: String,

    #[serde(default)]
    pub log: LogConfig,

    #[serde(default)]
    pub domains: Vec<DomainConfig>,
}

fn default_resolver() -> String {
    String::from("opendns")
}

impl Default for DnsConfig {
    fn default() -> Self {
        DnsConfig {
            ip_resolver: default_resolver(),
            log: Default::default(),
            domains: Default::default(),
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: LevelFilter,
}

fn default_log_level() -> LevelFilter {
    LevelFilter::Info
}

impl Default for LogConfig {
    fn default() -> LogConfig {
        LogConfig {
            level: default_log_level(),
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum DomainConfig {
    Cloudflare(CloudflareConfig),
    GoDaddy(GoDaddyConfig),
    Namecheap(NamecheapConfig),
    He(HeConfig),
    NoIp(NoIpConfig),
    Dynu(DynuConfig),
    Porkbun(PorkbunConfig),
}

impl DomainConfig {
    pub fn display_name(&self) -> String {
        match self {
            DomainConfig::Cloudflare(c) => format!("{} ({})", c.zone, "cloudflare"),
            DomainConfig::GoDaddy(c) => format!("{} ({})", c.domain, "godaddy"),
            DomainConfig::Namecheap(c) => format!("{} ({})", c.domain, "namecheap"),
            DomainConfig::He(c) => format!("{} ({})", c.hostname, "he"),
            DomainConfig::NoIp(c) => format!("{} ({})", c.hostname, "noip"),
            DomainConfig::Dynu(c) => format!("{} ({})", c.hostname, "dynu"),
            DomainConfig::Porkbun(c) => format!("{} ({})", c.domain, "porkbun"),
        }
    }

    pub fn get_ip_types(&self) -> Vec<IpType> {
        match self {
            DomainConfig::Cloudflare(cloudflare_config) => cloudflare_config.ip_types.clone(),
            _ => vec![IpType::V4],
        }
    }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum IpType {
    #[serde(rename = "4")]
    V4,
    #[serde(rename = "6")]
    V6,
}

fn ipv4_only() -> Vec<IpType> {
    vec![IpType::V4]
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct CloudflareConfig {
    pub email: Option<String>,
    pub key: Option<String>,
    pub token: Option<String>,
    pub zone: String,
    pub records: Vec<String>,
    #[serde(default = "ipv4_only")]
    pub ip_types: Vec<IpType>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GoDaddyConfig {
    #[serde(default = "godaddy_base_url")]
    pub base_url: String,
    pub key: String,
    pub secret: String,
    pub domain: String,
    pub records: Vec<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct NamecheapConfig {
    #[serde(default = "namecheap_base_url")]
    pub base_url: String,
    pub domain: String,
    pub ddns_password: String,
    pub records: Vec<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeConfig {
    #[serde(default = "he_base_url")]
    pub base_url: String,
    pub hostname: String,
    pub password: String,
    pub records: Vec<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct NoIpConfig {
    #[serde(default = "noip_base_url")]
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub hostname: String,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct DynuConfig {
    #[serde(default = "dynu_base_url")]
    pub base_url: String,
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub records: Vec<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct PorkbunConfig {
    #[serde(default = "porkbun_base_url")]
    pub base_url: String,
    pub domain: String,
    pub key: String,
    pub secret: String,
    pub records: Vec<String>,
}

fn godaddy_base_url() -> String {
    String::from("https://api.godaddy.com")
}

fn namecheap_base_url() -> String {
    String::from("https://dynamicdns.park-your-domain.com")
}

fn he_base_url() -> String {
    String::from("https://dyn.dns.he.net")
}

fn noip_base_url() -> String {
    String::from("https://dynupdate.no-ip.com")
}

fn dynu_base_url() -> String {
    String::from("https://api.dynu.com")
}

fn porkbun_base_url() -> String {
    String::from("https://api.porkbun.com/api/json/v3")
}

pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<DnsConfig, ConfigError> {
    let mut f = File::open(path).map_err(|e| ConfigError {
        kind: ConfigErrorKind::FileNotFound(e),
    })?;

    let mut contents = String::new();
    f.read_to_string(&mut contents).map_err(|e| ConfigError {
        kind: ConfigErrorKind::Misread(e),
    })?;

    let mut handlebars = Handlebars::new();

    handlebars
        .register_template_string("dness_config", contents)
        .map_err(|e| ConfigError {
            kind: ConfigErrorKind::Template(e),
        })?;
    handlebars.register_escape_fn(handlebars::no_escape);
    handlebars.set_strict_mode(true);

    let data: HashMap<_, _> = std::env::vars().collect();
    let config_contents = handlebars
        .render("dness_config", &data)
        .map_err(|e| ConfigError {
            kind: ConfigErrorKind::Render(e),
        })?;

    toml::from_str(&config_contents).map_err(|e| ConfigError {
        kind: ConfigErrorKind::Parse(e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_config_empty() {
        let config: DnsConfig = toml::from_str("").unwrap();
        assert_eq!(
            config,
            DnsConfig {
                ip_resolver: String::from("opendns"),
                log: LogConfig {
                    level: LevelFilter::Info,
                },
                domains: vec![]
            }
        )
    }

    #[test]
    fn deserialize_config_deny_unknown() {
        let err = toml::from_str::<DnsConfig>(r#"log_info = "DEBUG""#).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown field `log_info`"));
    }

    #[test]
    fn deserialize_config_simple() {
        let toml_str = &include_str!("../assets/base-config.toml");
        let config: DnsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DnsConfig {
                ip_resolver: String::from("opendns"),
                log: LogConfig {
                    level: LevelFilter::Info,
                },
                domains: vec![DomainConfig::Cloudflare(CloudflareConfig {
                    email: None,
                    key: None,
                    token: Some(String::from("dec0de")),
                    zone: String::from("example.com"),
                    records: vec![String::from("n.example.com")],
                    ip_types: vec![IpType::V4],
                })]
            }
        );
    }

    #[test]
    fn deserialize_config_ipv6() {
        let toml_str = &include_str!("../assets/ipv6-config.toml");
        let config: DnsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DnsConfig {
                ip_resolver: String::from("opendns"),
                log: LogConfig {
                    level: LevelFilter::Info,
                },
                domains: vec![DomainConfig::Cloudflare(CloudflareConfig {
                    email: None,
                    key: None,
                    token: Some(String::from("dec0de")),
                    zone: String::from("example.com"),
                    records: vec![String::from("n.example.com")],
                    ip_types: vec![IpType::V6],
                })]
            }
        );
    }

    #[test]
    fn deserialize_config_dual_stack() {
        let toml_str = &include_str!("../assets/dual-stack-config.toml");
        let config: DnsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DnsConfig {
                ip_resolver: String::from("opendns"),
                log: LogConfig {
                    level: LevelFilter::Info,
                },
                domains: vec![DomainConfig::Cloudflare(CloudflareConfig {
                    email: None,
                    key: None,
                    token: Some(String::from("dec0de")),
                    zone: String::from("example.com"),
                    records: vec![String::from("n.example.com")],
                    ip_types: vec![IpType::V4, IpType::V6],
                })]
            }
        )
    }

    #[test]
    fn deserialize_config_godaddy() {
        let toml_str = &include_str!("../assets/godaddy-config.toml");
        let config: DomainConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DomainConfig::GoDaddy(GoDaddyConfig {
                base_url: String::from("https://api.godaddy.com"),
                domain: String::from("example.com"),
                key: String::from("abc123"),
                secret: String::from("ef"),
                records: vec![String::from("@")]
            })
        );
    }

    #[test]
    fn deserialize_config_namecheap() {
        let toml_str = &include_str!("../assets/namecheap-config.toml");
        let config: DomainConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DomainConfig::Namecheap(NamecheapConfig {
                base_url: String::from("https://dynamicdns.park-your-domain.com"),
                domain: String::from("test-dness-1.xyz"),
                ddns_password: String::from("super_secret_password"),
                records: vec![String::from("@"), String::from("*"), String::from("sub")]
            })
        );
    }

    #[test]
    fn deserialize_config_he() {
        let toml_str = &include_str!("../assets/he-config.toml");
        let config: DomainConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DomainConfig::He(HeConfig {
                base_url: String::from("https://dyn.dns.he.net"),
                hostname: String::from("test-dness-1.xyz"),
                password: String::from("super_secret_password"),
                records: vec![String::from("@"), String::from("sub")]
            })
        );
    }

    #[test]
    fn deserialize_config_readme() {
        std::env::set_var("MY_CLOUDFLARE_TOKEN", "dec0de");
        let config = parse_config("assets/readme-config.toml").unwrap();
        assert_eq!(
            config,
            DnsConfig {
                ip_resolver: String::from("opendns"),
                log: LogConfig {
                    level: LevelFilter::Debug,
                },
                domains: vec![
                    DomainConfig::Cloudflare(CloudflareConfig {
                        email: None,
                        key: None,
                        token: Some(String::from("dec0de")),
                        zone: String::from("example.com"),
                        records: vec![String::from("n.example.com")],
                        ip_types: vec![IpType::V4],
                    }),
                    DomainConfig::Cloudflare(CloudflareConfig {
                        email: Some(String::from("admin@example.com")),
                        key: Some(String::from("deadbeef")),
                        token: None,
                        zone: String::from("example2.com"),
                        records: vec![
                            String::from("n.example2.com"),
                            String::from("n2.example2.com")
                        ],
                        ip_types: vec![IpType::V4],
                    })
                ]
            }
        );
    }

    #[test]
    fn deserialize_config_readme_bad() {
        let err = parse_config("assets/readme-config-bad.toml").unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("I_DO_NOT_EXIST"));
    }

    #[test]
    fn deserialize_ipify_config() {
        let toml_str = &include_str!("../assets/ipify-config.toml");
        let config: DnsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DnsConfig {
                ip_resolver: String::from("ipify"),
                log: LogConfig {
                    level: LevelFilter::Info,
                },
                domains: vec![]
            }
        );
    }

    #[test]
    fn deserialize_noip_config() {
        let toml_str = &include_str!("../assets/noip-config.toml");
        let config: DomainConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DomainConfig::NoIp(NoIpConfig {
                base_url: noip_base_url(),
                username: String::from("myemail@example.org"),
                hostname: String::from("dnesstest.hopto.org"),
                password: String::from("super_secret_password"),
            })
        );
    }

    #[test]
    fn deserialize_config_dynu() {
        let toml_str = &include_str!("../assets/dynu-config.toml");
        let config: DomainConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DomainConfig::Dynu(DynuConfig {
                base_url: String::from("https://api.dynu.com"),
                hostname: String::from("test-dness-1.xyz"),
                username: String::from("MyUserName"),
                password: String::from("IpUpdatePassword"),
                records: vec![String::from("@"), String::from("sub")]
            })
        );
    }
}
