mod cloudflare;
mod config;
mod core;
mod dns;
mod dynu;
mod errors;
mod godaddy;
mod he;
mod namecheap;
mod noip;
mod porkbun;

// Avoid musl's default allocator due to lackluster performance
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use crate::config::{parse_config, DnsConfig, DomainConfig, IpType};
use crate::core::Updates;
use crate::dns::wan_lookup_ip;
use crate::errors::DnessError;
use chrono::Duration;
use clap::Parser;
use log::{error, info, LevelFilter};
use std::error;
use std::fmt::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opt {
    /// Sets a custom config file
    #[structopt(short, long)]
    config: Option<PathBuf>,
}

fn log_err(context: &str, err: Box<dyn error::Error>) {
    let mut msg = String::new();
    let _ = writeln!(msg, "{} ", context);
    let _ = write!(msg, "\tcaused by: {}", err);

    let mut ie = err.source();
    while let Some(cause) = ie {
        let _ = write!(msg, "\n\tcaused by: {}", cause);
        ie = cause.source();
    }

    error!("{}", msg);
}

fn init_logging(lvl: LevelFilter) {
    env_logger::Builder::from_default_env()
        .filter_level(lvl)
        .target(env_logger::Target::Stdout)
        .init();
}

/// Parses the TOML configuration. If no configuration file is present, the default configuration
/// is returned so that the WAN IP can still be logged on execution. If there is an error parsing
/// the configuration file, exit with a non-zero status code.
fn init_configuration<T: AsRef<Path>>(file: Option<T>) -> DnsConfig {
    if let Some(config_file) = file {
        let path = config_file.as_ref();
        match parse_config(path) {
            Ok(c) => c,
            Err(e) => {
                // If there is an error during configuration, we assume a log level of Warn so that
                // the user will see the error printed.
                init_logging(LevelFilter::Warn);
                let desc = format!("could not configure application from: {}", path.display());
                log_err(&desc, Box::new(e));
                std::process::exit(1)
            }
        }
    } else {
        Default::default()
    }
}

async fn ipify_resolve_ip(client: &reqwest::Client, ip_type: IpType) -> Result<IpAddr, DnessError> {
    let ipify_url = match ip_type {
        IpType::V4 => "https://api.ipify.org/",
        IpType::V6 => "https://api6.ipify.org/",
    };
    let ip_text = client
        .get(ipify_url)
        .send()
        .await
        .map_err(|e| DnessError::send_http(ipify_url, "ipify get ip", e))?
        .error_for_status()
        .map_err(|e| DnessError::bad_response(ipify_url, "ipify get ip", e))?
        .text()
        .await
        .map_err(|e| DnessError::deserialize(ipify_url, "ipify get ip", e))?;

    let ip = ip_text
        .parse::<IpAddr>()
        .map_err(|_| DnessError::message(format!("unable to parse {} as an ip", &ip_text)))?;
    Ok(ip)
}

/// Resolves the WAN IP or exits with a non-zero status code
async fn resolve_ip(
    client: &reqwest::Client,
    config: &DnsConfig,
    ip_type: IpType,
) -> Result<IpAddr, DnessError> {
    match config.ip_resolver.to_ascii_lowercase().as_str() {
        "opendns" => wan_lookup_ip(ip_type).await.map_err(|x| x.into()),
        "ipify" => ipify_resolve_ip(client, ip_type).await,
        _ => {
            error!("unrecognized ip resolver: {}", config.ip_resolver);
            std::process::exit(1)
        }
    }
}

fn elapsed(start: Instant) -> String {
    Duration::from_std(Instant::now().duration_since(start))
        .map(|x| format!("{}ms", x.num_milliseconds()))
        .unwrap_or_else(|_| String::from("<error>"))
}

async fn update_provider(
    http_client: &reqwest::Client,
    addr: IpAddr,
    domain: &DomainConfig,
) -> Result<Updates, Box<dyn std::error::Error>> {
    match domain {
        DomainConfig::Cloudflare(domain_config) => {
            cloudflare::update_domains(http_client, domain_config, addr)
                .await
                .map_err(|e| e.into())
        }
        DomainConfig::GoDaddy(domain_config) => {
            godaddy::update_domains(http_client, domain_config, addr)
                .await
                .map_err(|e| e.into())
        }
        DomainConfig::Namecheap(domain_config) => {
            namecheap::update_domains(http_client, domain_config, addr)
                .await
                .map_err(|e| e.into())
        }
        DomainConfig::He(domain_config) => he::update_domains(http_client, domain_config, addr)
            .await
            .map_err(|e| e.into()),
        DomainConfig::NoIp(domain_config) => noip::update_domains(http_client, domain_config, addr)
            .await
            .map_err(|e| e.into()),
        DomainConfig::Dynu(domain_config) => dynu::update_domains(http_client, domain_config, addr)
            .await
            .map_err(|e| e.into()),
        DomainConfig::Porkbun(domain_config) => {
            porkbun::update_domains(http_client, domain_config, addr)
                .await
                .map_err(|e| e.into())
        }
    }
}

#[tokio::main]
async fn main() {
    let start = Instant::now();
    let opt = Opt::parse();
    let config = init_configuration(opt.config.as_ref());

    init_logging(config.log.level);

    // Use a single HTTP client when updating dns records so that connections can be reused
    let http_client = reqwest::Client::new();

    let mut ip_types: Vec<IpType> = if config.domains.is_empty() {
        vec![IpType::V4, IpType::V6]
    } else {
        config
            .domains
            .iter()
            .flat_map(|d| d.get_ip_types())
            .collect()
    };
    ip_types.sort_unstable();
    ip_types.dedup();
    let ip_types = ip_types;

    // Keep track of any failures in ensuring current DNS records. We don't want to fail on the
    // first error, as subsequent domains listed in the config can still be valid, but if there
    // were any failures, we still need to exit with a non-zero exit code
    let mut failure = false;

    let addrs: Vec<Option<IpAddr>> =
        futures::future::join_all(ip_types.iter().map(async |ip_type| {
            let start_resolve = Instant::now();
            match resolve_ip(&http_client, &config, *ip_type).await {
                Ok(addr) => {
                    info!("resolved address to {} in {}", addr, elapsed(start_resolve));
                    Some(addr)
                }
                Err(e) => {
                    log_err("could not successfully resolve IP", Box::new(e));
                    None
                }
            }
        }))
        .await;
    if addrs.iter().any(Option::is_none) {
        failure = true;
    }
    let addrs: Vec<IpAddr> = addrs.iter().copied().flatten().collect();

    let mut total_updates = Updates::default();

    for d in config.domains {
        let ip_types = d.get_ip_types();
        for addr in addrs.iter() {
            if !ip_types.contains(&IpType::from(*addr)) {
                continue;
            }
            let start_update = Instant::now();
            match update_provider(&http_client, *addr, &d).await {
                Ok(updates) => {
                    info!(
                        "processed {}: ({}) in {}",
                        d.display_name(),
                        updates,
                        elapsed(start_update)
                    );
                    total_updates += updates;
                }
                Err(e) => {
                    failure = true;
                    let msg = format!("could not update {}", d.display_name(),);
                    log_err(&msg, e);
                }
            }
        }
    }

    info!("processed all: ({}) in {}", total_updates, elapsed(start));
    if failure {
        error!("at least one update failed, so exiting with non-zero status code");
        std::process::exit(1)
    }
}
