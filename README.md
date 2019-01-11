# dness

***Finesse with dness: a dynamic dns client***

[![Build Status](https://travis-ci.org/nickbabcock/dness.svg?branch=master)](https://travis-ci.org/nickbabcock/dness) [![Build status](https://ci.appveyor.com/api/projects/status/dic59x43w2g19536?svg=true)](https://ci.appveyor.com/project/nickbabcock/dness)

---

## Motivation

When one has a server that is subjected to unpredictable IP address changes, such as at home or elsewhere, a change in IP address causes unexpected downtime. Instead of paying for a static IP address, one can employ a dynamic dns client on said server, which will update the [WAN](https://en.wikipedia.org/wiki/Wide_area_network) IP address on the dns server.

There are plenty of dynamic dns clients, including the venerable [ddclient](https://github.com/ddclient/ddclient), but troublesome installation + perl system dependency resolution, and cache format errors have left much to be desired. Other solutions fall short, so dness was created with the following goals:

**Goals**:

- Cross platform (Windows, Mac, Linux, ARM, BSD)
- "zero" dependencies
  - Depend on only already installed system wide dependencies (libssl (eg: openssl))
  - And offer statically linked builds for truly zero dependencies
- A standard configuration ([TOML](https://github.com/toml-lang/toml)) that is similar to ddclient's.
- Extendable to allow for more dynamic dns services
- Sensible logging to glean insight into inevitable problems that arise with networked services
- Permissively licensed

## Installation

To maximize initial flexibility, dness is not a daemon. Instead it relies on the host's scheduling (cron, systemd timers, windows scheduler).

### Ubuntu / Debian (systemd + deb)

- Decide if you want a static musl package or one that depends on the system's openssl.
- Download the [latest chosen deb](https://github.com/nickbabcock/dness/releases/latest)

```bash
dpkg -i dness<version>_amd64.deb

# ensure it is working
/usr/bin/dness

# enable systemd timer
systemctl daemon-reload
systemctl start dness.timer
systemctl enable dness.timer

# update configuration
${EDITOR:-vi} /etc/dness/dness.conf
```

### Linux Musl

The linux musl build is a static build of dness. It has zero dependencies. Nothing is quite like scp'ing or curl'ing a musl binary to a random / unknown linux server and having it just work.

- Download the [latest "-x86_64-unknown-linux-musl.tar.gz" ](https://github.com/nickbabcock/dness/releases/latest)
- untar (`tar -xzf *-x86_64-unknown-linux-musl.tar.gz`)
- enjoy

### Windows

- Download the [latest ".exe"](https://github.com/nickbabcock/dness/releases/latest)
- Create configuration file (`dness.conf`)
- Execute `dness.exe -c dness.conf` to verify behavior
- If desired, use windows task scheduler to execute command at specific times

### Other

Download the [latest appropriate target](https://github.com/nickbabcock/dness/releases/latest)

## Configuration

No configuration file is necessary when only the WAN IP is desired.

```bash
./dness
```

Sample output:

```
[INFO  trust_dns_proto::xfer::dns_exchange] sending message via: UDP(208.67.220.220:53)
[INFO  dness] resolved address to 256.256.256.256 in 23ms
[INFO  dness] processed all: (updated: 0, already current: 0, missing: 0) in 29ms
```

### Sample Configuration

But dness can do more than resolve one's WAN IP. Below is a sample configuration (dness.conf) that should cover most needs:

```toml
[log]
# How verbose the log is. Commons values are Error, Warn, Info, Debug, Trace
level = "Debug"

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

# More than one domain can be specified in a config!
[[domains]]
type = "cloudflare"
email = "admin@example.com"
key = "deadbeef"
zone = "example2.com"
records = [
    "n.example2.com",
    "n2.example2.com"
]
```

Execute with configuration:

```
./dness -c dness.conf
```

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

No configuration option are available for OpenDNS. It resolves IPv4 addresses by querying "myip.opendns.com" against resolver1.opendns.com and resolver2.opendns.com.
