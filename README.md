# Cloudflare Dynamic DNS Updater

A lightweight Rust application that automatically updates Cloudflare DNS records with your current public IP address. Perfect for home servers and dynamic IP environments.

## Features

- Automatically detects your public IP address
- Updates multiple subdomains in a single run
- Supports Cloudflare proxy settings per subdomain
- Simple YAML configuration
- Minimal resource usage

## Prerequisites

- Rust (1.70 or later)
- A Cloudflare account with a domain
- Cloudflare API token with DNS write permissions

## Installation

```bash
git clone https://github.com/yourusername/cloudflare-ddns
cd cloudflare-ddns
cargo build --release
```

## Configuration

Create a `config.yml` file in the same directory as the executable:

```yaml
api_token: "your-cloudflare-api-token"
zone_id: "your-zone-id"
ttl: 3600
subdomains:
  - name: ""          # Use empty string or "@" for root domain
    proxied: false
  - name: "home"
    proxied: false
  - name: "vpn"
    proxied: false
```

### Getting Your Credentials

1. **API Token**: Go to Cloudflare Dashboard → My Profile → API Tokens → Create Token
   - Use the "Edit zone DNS" template
   - Select your specific zone
   - Save the token securely

2. **Zone ID**: Found in your domain's overview page on the Cloudflare dashboard

## Usage

Run the application:

```bash
./cloudflare-ddns
```

Example output:
```
Current IP: 73.172.10.94
Setting IP of @ to 73.172.10.94
Setting IP of home to 73.172.10.94
Setting IP of vpn to 73.172.10.94
```

### Automation

Set up a cron job to run every 5 minutes:

```bash
*/5 * * * * /path/to/cloudflare-ddns >> /var/log/cloudflare-ddns.log 2>&1
```

Or create a systemd timer for more control.

## Notes

- The application only updates A records (IPv4)
- DNS records must already exist in Cloudflare - this tool updates them, it doesn't create new ones
- The `proxied` setting determines whether traffic goes through Cloudflare's CDN

## License

MIT

