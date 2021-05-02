mod cloudflare;
mod config;
mod core;
mod dns;
mod errors;
mod godaddy;
mod he;
mod namecheap;
mod noip;

use crate::config::{parse_config, DnsConfig, DomainConfig};
use crate::core::Updates;
use crate::dns::wan_lookup_ip;
use crate::errors::DnessError;
use chrono::Duration;
use log::{error, info, LevelFilter};
use std::error;
use std::fmt::Write;
use std::net::Ipv4Addr;
use std::time::Instant;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "dness")]
struct Opt {
    #[structopt(short = "c", long = "config")]
    config_file: Option<String>,
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
fn init_configuration(file: &Option<String>) -> DnsConfig {
    if let Some(ref config_file) = file {
        match parse_config(&config_file) {
            Ok(c) => c,
            Err(e) => {
                // If there is an error during configuration, we assume a log level of Warn so that
                // the user will see the error printed.
                init_logging(LevelFilter::Warn);
                let desc = format!("could not configure application from: {}", &config_file);
                log_err(&desc, Box::new(e));
                std::process::exit(1)
            }
        }
    } else {
        Default::default()
    }
}

async fn ipify_resolve_ip(client: &reqwest::Client) -> Result<Ipv4Addr, DnessError> {
    let ipify_url = "https://api.ipify.org/";
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
        .parse::<Ipv4Addr>()
        .map_err(|_| DnessError::message(format!("unable to parse {} as an ip", &ip_text)))?;
    Ok(ip)
}

/// Resolves the WAN IP or exits with a non-zero status code
async fn resolve_ip(client: &reqwest::Client, config: &DnsConfig) -> Ipv4Addr {
    let res = match config.ip_resolver.to_ascii_lowercase().as_str() {
        "opendns" => wan_lookup_ip().await.map_err(|x| x.into()),
        "ipify" => ipify_resolve_ip(&client).await,
        _ => {
            error!("unrecognized ip resolver: {}", config.ip_resolver);
            std::process::exit(1)
        }
    };

    match res {
        Ok(c) => c,
        Err(e) => {
            log_err("could not successfully resolve IP", Box::new(e));
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
    addr: Ipv4Addr,
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
    }
}

#[tokio::main]
async fn main() {
    let start = Instant::now();
    let opt = Opt::from_args();
    let config = init_configuration(&opt.config_file);

    init_logging(config.log.level);

    // Use a single HTTP client when updating dns records so that connections can be reused
    let http_client = reqwest::Client::new();

    let start_resolve = Instant::now();
    let addr = resolve_ip(&http_client, &config).await;
    info!("resolved address to {} in {}", addr, elapsed(start_resolve));

    // Keep track of any failures in ensuring current DNS records. We don't want to fail on the
    // first error, as subsequent domains listed in the config can still be valid, but if there
    // were any failures, we still need to exit with a non-zero exit code
    let mut failure = false;
    let mut total_updates = Updates::default();

    for d in config.domains {
        let start_update = Instant::now();
        match update_provider(&http_client, addr, &d).await {
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

    info!("processed all: ({}) in {}", total_updates, elapsed(start));
    if failure {
        error!("at least one update failed, so exiting with non-zero status code");
        std::process::exit(1)
    }
}
