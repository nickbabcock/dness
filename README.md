# dness

***Finesse with dness: a dynamic dns client***

---

## Motivation

When one has a server that is subjected to unpredictable IP address changes, such as at home or elsewhere, one experiences unexpected downtime. Instead of paying for a static IP address, one can employ a dynamic dns client on said server, which will update the IP address on the dns server.

There are plenty of dynamic dns clients, including the venerable [ddclient](https://github.com/ddclient/ddclient), but troublesome installation + dependency resolution, and cache format errors have left much to be desired. Other solutions fall short, so dness was created with the following goals:

**Goals**:

- Cross platform (Windows, Mac, Linux, ARM, BSD)
- "zero" dependencies
  - Depend on only already installed system wide dependencies (libc, libcrypto, libssl)
  - And offer statically linked builds for truly zero dependencies
- A standard configuration ([TOML](https://github.com/toml-lang/toml)) that is similar to ddclient's.
- Extendable to allow for more dynamic dns services
- Sensible logging to glean insight into inevitable problems that arise with networked services
- Permissively licensed

## Installation

To maximize initial flexibility, dness is not a daemon. Instead it relies on the host's scheduling (cron, systemd timers, windows scheduler). Future updates may add daemon functionality.

## Configuration

### Supported Dynamic DNS Services

#### Cloudflare

```toml
[[domains]]
# We denote that our domain is managed by cloudflare
type = "cloudflare"

# The email address registered in cloudflare that is authorized to update dns
# records
email = "admin@example.com"

# The cloudflare key can be found in the domain overview, in "Get your API key"
# and view "Global API Key" (or another key as appropriate)
key = "deadbeef"

# The zone is the domain name
zone = "example.com"

# List of A records found under the DNS tab that should be updated
records = [
    "n.example.com"
]
```

Cloudflare dynamic dns service works in three steps:

1. Send GET to translate the zone (example.com) to cloudflare's id
2. Send GET to find all the domains under the zone and their sub-ids
  - Cloudflare paginates the response to handle many subdomains
  - It is possible to query for individual domains but as long as more
    than one desired domain in each page -- this methods cuts down requests
3. Each desired domain in the config is checked to ensure that it is set to our address. In
   this way cloudflare is our cache (to guard against nefarious users updating out of band)

### Supported WAN IP Resolvers

No other WAN IP resolvers are available, but it certainly possible to add other DNS or HTTP resolvers in the future.

#### OpenDNS

No configuration option are available for OpenDNS.
