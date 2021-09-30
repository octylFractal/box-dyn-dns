use std::net::Ipv4Addr;

use attohttpc::Response;
use color_eyre::eyre::{eyre, WrapErr};
use log::info;
use serde::{Deserialize, Serialize};

use crate::update_dns::api::{UpdateDns, UpdateDnsCreator};

pub struct Cloudflare {
    config: CloudflareConfig,
}

#[derive(Deserialize, Debug)]
pub struct CloudflareConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    pub zone_id: String,
    pub api_token: String,
}

fn default_base_url() -> String {
    "https://api.cloudflare.com/client/v4".to_string()
}

impl Cloudflare {
    fn create_cf_error(response: Response) -> color_eyre::Report {
        eyre!(
            "{status} Error from Cloudflare: {de:?}",
            status = response.status(),
            de = response.json::<CloudflareResponse<()>>().map_or_else(
                |e| format!("Unable to read response: {:?}", e),
                |v| format!("{:?}", v.errors),
            ),
        )
    }
}

impl UpdateDnsCreator for Cloudflare {
    type Config = CloudflareConfig;

    fn from_config(config: Self::Config) -> Self {
        Cloudflare { config }
    }
}

impl UpdateDns for Cloudflare {
    fn describe(&self) -> String {
        format!("Cloudflare[zone={zone_id}]", zone_id = &self.config.zone_id)
    }

    fn update_dns(&self, name: String, new_ip: Ipv4Addr) -> color_eyre::Result<()> {
        // GET all `name` `A` records
        let response = attohttpc::get(format!(
            "{base}/zones/{zone_id}/dns_records",
            base = self.config.base_url,
            zone_id = &self.config.zone_id,
        ))
        .param("name", &name)
        .param("type", "A")
        .header("Authorization", format!("Bearer {}", self.config.api_token))
        .send()
        .wrap_err("Failed to send request")?;
        if !response.is_success() {
            return Err(Cloudflare::create_cf_error(response));
        }

        let cf_res: CloudflareResponse<Vec<CloudflareListDnsRecordRes>> =
            response.json().wrap_err("Failed to read response")?;
        assert!(
            cf_res.success && cf_res.result.is_some(),
            "Not successful or no result: {:?}",
            cf_res
        );
        let list = cf_res.result.unwrap();
        let record = match list.as_slice() {
            [r] => r,
            _ => return Err(eyre!("Expected exactly one result, got {:?}", list)),
        };

        if record.content == new_ip.to_string() {
            info!("[cloudflare] New IP is the same as existing, skipping update.");
            return Ok(());
        }

        info!("[cloudflare] Old content was {}", record.content);

        let response = attohttpc::put(format!(
            "{base}/zones/{zone_id}/dns_records/{id}",
            base = self.config.base_url,
            zone_id = &self.config.zone_id,
            id = record.id,
        ))
        .json(&CloudflareUpdateDnsRecordReq {
            record_type: "A".to_string(),
            name: record.name.to_string(),
            content: new_ip.to_string(),
            ttl: record.ttl,
        })
        .wrap_err("Failed to serialize body")?
        .header("Authorization", format!("Bearer {}", self.config.api_token))
        .send()
        .wrap_err("Failed to send request")?;
        if !response.is_success() {
            return Err(Cloudflare::create_cf_error(response));
        }

        let cf_res: CloudflareResponse<serde_json::Value> =
            response.json().wrap_err("Failed to read response")?;
        assert!(cf_res.success, "Not successful: {:?}", cf_res);
        info!("Successful: {:?}", cf_res);

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct CloudflareResponse<T> {
    result: Option<T>,
    success: bool,
    errors: Vec<CloudflareError>,
}

#[derive(Deserialize, Debug)]
struct CloudflareError {
    code: u32,
    message: String,
}

#[derive(Deserialize, Debug)]
struct CloudflareListDnsRecordRes {
    id: String,
    name: String,
    content: String,
    ttl: u32,
}

#[derive(Serialize)]
struct CloudflareUpdateDnsRecordReq {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: u32,
}
