use std::net::{IpAddr, Ipv4Addr};

use color_eyre::eyre::{ContextCompat, WrapErr};
use log::info;
use serde::Deserialize;
use structopt::StructOpt;
use trust_dns_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use trust_dns_resolver::Resolver;

use crate::update_dns::api::{UpdateDns, UpdateDnsCreator};
use crate::update_dns::cloudflare::{Cloudflare, CloudflareConfig};

mod update_dns;

const RUST_BACKTRACE: &str = "RUST_BACKTRACE";

#[derive(StructOpt, Debug)]
pub(crate) struct BoxDynDns {
    /// Verbosity of output, 1 occurrence for debug, 2 occurrences for trace
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: usize,
}

fn main() -> color_eyre::Result<()> {
    if std::env::var_os(RUST_BACKTRACE).is_none() {
        std::env::set_var(RUST_BACKTRACE, "1");
    }

    let args: BoxDynDns = BoxDynDns::from_args();

    color_eyre::install()?;
    stderrlog::new()
        .verbosity(args.verbose + 2)
        .init()
        .wrap_err("Failed to initialize logging")?;

    let config = load_config()?;

    let resolver = Resolver::new(
        ResolverConfig::from_parts(
            None,
            vec![],
            NameServerConfigGroup::from_ips_clear(
                &[IpAddr::V4(Ipv4Addr::new(208, 67, 222, 222))],
                53,
                true,
            ),
        ),
        ResolverOpts::default(),
    )
    .wrap_err("Failed to initialize resolver")?;
    let response = resolver
        .lookup_ip("myip.opendns.com.")
        .wrap_err("Failed to resolve IP address")?;
    let address = response
        .iter()
        .filter_map(|x| match x {
            IpAddr::V4(v4) => Some(v4),
            _ => None,
        })
        .next()
        .wrap_err("No IPv4 addresses returned")?;

    info!("Your public IP address is {}", address);

    let update_dns: Box<dyn UpdateDns> = config.update_dns.into();

    info!(
        "Attempting to update DNS entry with {}",
        update_dns.describe()
    );

    update_dns
        .update_dns(config.dns_name, address)
        .wrap_err("Failed to update DNS entry")?;

    Ok(())
}

fn load_config() -> color_eyre::Result<Secrets> {
    serde_yaml::from_reader(std::fs::File::open("./secrets.yml")?)
        .wrap_err("Failed to read secrets")
}

#[derive(Deserialize, Debug)]
struct Secrets {
    dns_name: String,
    update_dns: UpdateDnsConfig,
}

#[derive(Deserialize, Debug)]
enum UpdateDnsConfig {
    #[serde(rename = "cloudflare")]
    Cloudflare(CloudflareConfig),
}

impl From<UpdateDnsConfig> for Box<dyn UpdateDns> {
    fn from(config: UpdateDnsConfig) -> Box<dyn UpdateDns> {
        match config {
            UpdateDnsConfig::Cloudflare(cf) => Box::from(Cloudflare::from_config(cf)),
        }
    }
}
