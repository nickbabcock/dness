extern crate reqwest;
extern crate serde;
extern crate trust_dns_resolver;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate structopt;
extern crate toml;
#[macro_use]
extern crate log;
extern crate chrono;
extern crate env_logger;
extern crate failure;

mod cloudflare;
mod config;
mod dns;
mod godaddy;
mod iplookup;

use crate::config::{parse_config, DnsConfig, DomainConfig};
use crate::iplookup::lookup_ip;

use crate::dns::Updates;
use chrono::Duration;
use log::LevelFilter;
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

fn log_err<E: error::Error>(context: &str, err: &E) {
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
                log_err(&desc, &e);
                std::process::exit(1)
            }
        }
    } else {
        Default::default()
    }
}

/// Resolves the WAN IP or exits with a non-zero status code
fn resolve_ip() -> Ipv4Addr {
    match lookup_ip() {
        Ok(c) => c,
        Err(e) => {
            log_err("could not successfully resolve IP", &e);
            std::process::exit(1)
        }
    }
}

fn main() {
    let start = Instant::now();
    let opt = Opt::from_args();
    let config = init_configuration(&opt.config_file);

    init_logging(config.log.level);

    let start_resolve = Instant::now();
    let addr = resolve_ip();
    info!("resolved address to {} in {}", addr, elapsed(start_resolve));

    // Use a single HTTP client when updating dns records so that connections can be reused
    let http_client = reqwest::Client::new();

    // Keep track of any failures in ensuring current DNS records. We don't want to fail on the
    // first error, as subsequent domains listed in the config can still be valid, but if there
    // were any failures, we still need to exit with a non-zero exit code
    let mut failure = false;
    let mut total_updates = Updates::default();

    for d in config.domains {
        match d {
            DomainConfig::Cloudflare(domain_config) => {
                let start_cloudflare = Instant::now();
                match cloudflare::update_domains(&http_client, &domain_config, addr) {
                    Ok(updates) => {
                        info!(
                            "processed cloudflare: {} ({}) in {}",
                            domain_config.zone,
                            updates,
                            elapsed(start_cloudflare)
                        );
                        total_updates += updates;
                    }
                    Err(ref e) => {
                        failure = true;
                        let msg = format!(
                            "could not update cloudflare domains in: {}",
                            domain_config.zone
                        );
                        log_err(&msg, e);
                    }
                }
            }
            DomainConfig::GoDaddy(domain_config) => {
                let start_godaddy = Instant::now();
                match godaddy::update_domains(&http_client, &domain_config, addr) {
                    Ok(updates) => {
                        info!(
                            "processed godaddy: {} ({}) in {}",
                            domain_config.domain,
                            updates,
                            elapsed(start_godaddy)
                        );
                        total_updates += updates;
                    }
                    Err(ref e) => {
                        failure = true;
                        let msg = format!(
                            "could not update godaddy domains in: {}",
                            domain_config.domain
                        );
                        log_err(&msg, e);
                    }
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

fn elapsed(start: Instant) -> String {
    Duration::from_std(Instant::now().duration_since(start))
        .map(|x| format!("{}ms", x.num_milliseconds()))
        .unwrap_or_else(|_| String::from("<error>"))
}
