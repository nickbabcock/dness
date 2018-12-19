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

- Cloudflare

### Supported WAN IP Resolvers

- OpenDNS
