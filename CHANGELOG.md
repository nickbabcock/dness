## 0.1.0 - 2019-01-11

This is the initial release of dness -- and it is currently only an MVP (minimal viable project). Dness does one thing: detect [WAN IP](https://en.wikipedia.org/wiki/Wide_area_network) through OpenDNS and update the appropriate records on Cloudflare. But already at v0.1.0 it has scratched my itch; solved a problem I had with the current array of dynamic dns clients, so I decided to release it -- not in the thought that dness will be some de facto dynamic dns client, but that if dness solved a problem I had, maybe it will solve others' problems.

With that said, there here are a list of improvements that can conceivably be implemented:

- Support dynamic dns in the truest / traditional sense of the phrase by supporting [rfc2136 (DNS Update)](https://tools.ietf.org/html/rfc2136)
- Support additional dns hosts (eg: namecheap)
- Support additional ip resolvers (eg: http://httpbin.org/ip)
- Multiplex requests / operations using tokio
- Allow daemon mode so dness is more self contained
- Additional packaging (APT / yum repos)
- Configurable logging (ie: json) to be flexible enough to meet any logging needs
