use serde::{Deserialize, Serialize};
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
    #[serde(rename = "type")]
    ty: String,
    name: String,
    content: String,
    ttl: usize,
    proxied: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiMessage<T> {
    success: bool,
    errors: Vec<ApiError>,
    messages: Vec<ApiError>,
    result: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiError {
    code: u32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    documentation_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DnsRecord {
    id: String,
    #[serde(rename = "type")]
    ty: String,
    name: String,
    content: String,
    proxied: bool,
    ttl: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxiable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListResponse {
    result: Vec<DnsRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_info: Option<ResultInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResultInfo {
    count: u32,
    page: u32,
    per_page: u32,
    total_count: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let f = std::fs::File::open(CONFIG_FILE)?;
    let mut config: Config = serde_yaml::from_reader(f)?;
    
    // Convert empty subdomain names to "@"
    config.subdomains.iter_mut().for_each(|sd| {
        if sd.name.is_empty() {
            sd.name = "@".to_string();
        }
    });

    let ip = get_ip().await?;
    println!("Current IP: {}", ip);

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
        .find_map(|s| match s.split_once('=') {
            Some(("ip", x)) => Some(x.to_string()),
            _ => None,
        })
        .ok_or_else(|| "No IP found.".into())
}

async fn match_subdomain_ids(config: &mut Config) -> Result<(), Box<dyn Error>> {
    let req = format!("{}/zones/{}/dns_records?type=A", API_BASE, config.zone_id);

    let client = reqwest::Client::new();
    let response = client
        .get(&req)
        .bearer_auth(&config.api_token)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("API request failed: {}", response.status()).into());
    }

    let records: ApiMessage<Vec<DnsRecord>> = response.json().await?;

    if let Some(results) = records.result {
        for subdomain in &mut config.subdomains {
            subdomain.id = results.iter().find_map(|record| {
                if subdomain.name == "@" {
                    // For root domain, check if record name matches zone name
                    if !record.name.contains('.') || record.name.split('.').count() == 2 {
                        Some(record.id.clone())
                    } else {
                        None
                    }
                } else {
                    // For subdomains, check if record name starts with subdomain
                    if record.name.starts_with(&subdomain.name) {
                        Some(record.id.clone())
                    } else {
                        None
                    }
                }
            });
        }
    }

    Ok(())
}

async fn update_dns(ip: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();

    for sd in &config.subdomains {
        let Some(id) = &sd.id else {
            eprintln!("Skipping {} - no matching DNS record found", sd.name);
            continue;
        };

        println!("Setting IP of {} to {}", sd.name, ip);

        let req = format!(
            "{}/zones/{}/dns_records/{}",
            API_BASE, config.zone_id, id
        );

        let update_data = UpdateRecord {
            ty: "A".to_string(),
            name: sd.name.clone(),
            content: ip.to_string(),
            ttl: config.ttl,
            proxied: sd.proxied,
        };

        let response = client
            .patch(&req)
            .bearer_auth(&config.api_token)
            .json(&update_data)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to update {}: {}", sd.name, response.status()).into());
        }

        let result: ApiMessage<DnsRecord> = response.json().await?;
        
        if !result.success {
            for error in &result.errors {
                eprintln!("Error {}: {}", error.code, error.message);
            }
            return Err(format!("Failed to update DNS record for {}", sd.name).into());
        }
    }

    Ok(())
}

