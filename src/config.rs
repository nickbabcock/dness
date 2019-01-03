use log::LevelFilter;
use std::error;
use std::fmt;
use std::fs::File;
use std::io::Error as IoError;
use std::io::Read;
use std::path::Path;

#[derive(Debug)]
pub struct ConfigError {
    kind: ConfigErrorKind,
}

#[derive(Debug)]
pub enum ConfigErrorKind {
    FileNotFound(IoError),
    Misread(IoError),
    Parse(toml::de::Error),
}

impl error::Error for ConfigError {
    fn cause(&self) -> Option<&error::Error> {
        match self.kind {
            ConfigErrorKind::FileNotFound(ref e) => Some(e),
            ConfigErrorKind::Misread(ref e) => Some(e),
            ConfigErrorKind::Parse(ref e) => Some(e),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "config issue: ")?;
        match self.kind {
            ConfigErrorKind::FileNotFound(ref _e) => write!(f, "file not found"),
            ConfigErrorKind::Misread(ref _e) => write!(f, "unable to read file"),
            ConfigErrorKind::Parse(ref _e) => write!(f, "a parsing error"),
        }
    }
}

fn default_log_level() -> LevelFilter {
    LevelFilter::Info
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct DnsConfig {
    #[serde(default = "default_log_level")]
    pub log_level: LevelFilter,
    pub domains: Vec<DomainConfig>,
}

impl Default for DnsConfig {
    fn default() -> DnsConfig {
        DnsConfig {
            log_level: default_log_level(),
            domains: Default::default(),
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum DomainConfig {
    Cloudflare(CloudflareConfig),
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct CloudflareConfig {
    pub email: String,
    pub key: String,
    pub zone: String,
    pub records: Vec<String>,
}

pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<DnsConfig, ConfigError> {
    let mut f = File::open(path).map_err(|e| ConfigError {
        kind: ConfigErrorKind::FileNotFound(e),
    })?;

    let mut contents = String::new();
    f.read_to_string(&mut contents).map_err(|e| ConfigError {
        kind: ConfigErrorKind::Misread(e),
    })?;

    toml::from_str(&contents).map_err(|e| ConfigError {
        kind: ConfigErrorKind::Parse(e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_config_simple() {
        let toml_str = &include_str!("../assets/base-config.toml");
        let config: DnsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DnsConfig {
                log_level: LevelFilter::Info,
                domains: vec![DomainConfig::Cloudflare(CloudflareConfig {
                    email: String::from("a@b.com"),
                    key: String::from("deadbeef"),
                    zone: String::from("example.com"),
                    records: vec![String::from("n.example.com")]
                })]
            }
        );
    }

    #[test]
    fn deserialize_config_readme() {
        let toml_str = &include_str!("../assets/readme-config.toml");
        let config: DnsConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config,
            DnsConfig {
                log_level: LevelFilter::Warn,
                domains: vec![
                    DomainConfig::Cloudflare(CloudflareConfig {
                        email: String::from("admin@example.com"),
                        key: String::from("deadbeef"),
                        zone: String::from("example.com"),
                        records: vec![String::from("n.example.com")]
                    }),
                    DomainConfig::Cloudflare(CloudflareConfig {
                        email: String::from("admin@example.com"),
                        key: String::from("deadbeef"),
                        zone: String::from("example2.com"),
                        records: vec![
                            String::from("n.example2.com"),
                            String::from("n2.example2.com")
                        ]
                    })
                ]
            }
        );
    }
}
