use std::net::Ipv4Addr;

use serde::de::DeserializeOwned;

pub(crate) trait UpdateDnsCreator
where
    Self: UpdateDns,
{
    type Config: DeserializeOwned;

    fn from_config(config: Self::Config) -> Self;
}

pub(crate) trait UpdateDns {
    fn describe(&self) -> String;

    fn update_dns(&self, name: String, new_ip: Ipv4Addr) -> color_eyre::Result<()>;
}
