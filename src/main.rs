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
mod iplookup;

use config::{parse_config, DomainConfig};
use iplookup::lookup_ip;

use chrono::Duration;
use dns::Updates;
use std::error;
use std::fmt::Write;
use std::time::Instant;
use structopt::StructOpt;
use log::LevelFilter;

#[derive(StructOpt, Debug)]
#[structopt(name = "dnsess")]
struct Opt {
    #[structopt(short = "c", long = "config")]
    config_file: String,
}

fn log_err<E: error::Error>(context: &str, err: &E) {
    let mut msg = String::new();
    let _ = writeln!(msg, "{} ", context);
    let _ = writeln!(msg, "\tcaused by: {}", err);

    let mut ie = err.cause();
    while let Some(cause) = ie {
        let _ = writeln!(msg, "\tcaused by: {}", cause);
        ie = cause.cause();
    }

    error!("{}", msg);
}

fn init_logging(lvl: LevelFilter) {
    env_logger::Builder::from_default_env()
        .filter_level(lvl)
        .init();
}

fn main() {
    let start = Instant::now();
    let opt = Opt::from_args();

    // Parse the toml configuration file
    let config_file = &opt.config_file;
    let config = match parse_config(config_file) {
        Ok(c) => c,
        Err(e) => {
            init_logging(LevelFilter::Warn);
            let desc = format!("could not configure application from: {}", &config_file);
            log_err(&desc, &e);
            std::process::exit(1)
        }
    };

    // setup logging
    init_logging(config.log_level);

    // Resolve our WAN IP
    let start_resolve = Instant::now();
    let addr = match lookup_ip() {
        Ok(c) => c,
        Err(e) => {
            log_err("could not successfully resolve IP", &e);
            std::process::exit(1)
        }
    };
    info!("resolved address to {} in {}", addr, elapsed(start_resolve));

    let http_client = reqwest::Client::new();
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
