use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use std::{collections::HashMap, error::Error};

const API_BASE: &str = "https://api.cloudflare.com/client/v4";
const CONFIG_FILE: &str = "./config.yml";

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    api_token: String,
    zone_id: String,
    subdomains: Vec<Subdomain>,
    ttl: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Subdomain {
    name: String,
    proxied: bool,
    id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateRecord {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    ty: String,
    name: String,
    content: String,
    ttl: usize,
    proxied: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage {
    success: bool,
    errors: Vec<HashMap<String, Value>>,
    result: Option<OneOrMany>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResult {
    id: String,
    #[serde(rename(serialize = "type", deserialize = "type"))]
    ty: String,
    name: String,
    content: String,
    proxied: bool,
    ttl: usize,
    zone_id: String,
    zone_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(ApiResult),
    Vec(Vec<ApiResult>),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let f = std::fs::File::open(CONFIG_FILE)?;
    let mut config: Config = serde_yaml::from_reader(f)?;
    config.subdomains.iter_mut().for_each(|sd| {
        if sd.name == "" {
            sd.name = "@".to_string();
        }
    });

    let ip = get_ip().await?;
    println!("Current ip: {}", ip);

    match_subdomain_ids(&mut config).await?;

    update_dns(&ip, &config).await?;

    Ok(())
}

async fn get_ip() -> Result<String, Box<dyn Error>> {
    let resp = reqwest::get("https://1.1.1.1/cdn-cgi/trace")
        .await?
        .text()
        .await?;

    resp.split_ascii_whitespace()
        .find_map(|s| match s.split_once("=") {
            Some(("ip", x)) => Some(x.to_string()),
            _ => None,
        })
        .ok_or_else(|| "No ip found.".into())
}

async fn match_subdomain_ids(config: &mut Config) -> Result<(), Box<dyn Error>> {
    let req = format!("{}/zones/{}/dns_records?type=A", API_BASE, config.zone_id);

    let client = reqwest::Client::new();
    let records = client
        .get(req)
        .bearer_auth(&config.api_token)
        .send()
        .await?
        .json::<ApiMessage>()
        .await?;

    if let Some(OneOrMany::Vec(ref results)) = records.result {
        config
            .subdomains
            .iter_mut()
            .for_each(|sd| match sd.name.as_str() {
                "@" => {
                    sd.id = results.iter().find_map(|r| {
                        if r.zone_name == r.name {
                            Some(r.id.clone())
                        } else {
                            None
                        }
                    });
                }
                _ => {
                    sd.id = results.iter().find_map(|r| {
                        if r.name.starts_with(&sd.name) {
                            Some(r.id.clone())
                        } else {
                            None
                        }
                    });
                }
            })
    }

    Ok(())
}

async fn update_dns(ip: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();

    for sd in &config.subdomains {
        println!("Setting ip of {} to {}", sd.name.as_str(), ip);

        let req = format!(
            "{}/zones/{}/dns_records/{}",
            API_BASE,
            config.zone_id,
            sd.id.as_ref().unwrap()
        );

        let map = UpdateRecord {
            ty: "A".to_string(),
            name: sd.name.clone(),
            content: ip.to_string(),
            ttl: 1,
            proxied: sd.proxied,
        };

        let res = client
            .put(req)
            .bearer_auth(&config.api_token)
            .json(&map)
            .send()
            .await?
            .json::<ApiMessage>()
            .await?;
        if !res.errors.is_empty() {
            res.errors.iter().for_each(|e| eprintln!("{:#?}", e));
            panic!("Errors submitting update.")
        }
    }

    Ok(())
}
