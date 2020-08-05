## 0.3.2 - 2020-08-04

- Fixed build system used to generate binaries

## 0.3.1 - 2020-08-04

- Fixed cloudflare DNS updates resetting entries to their default values (eg: if a record was marked as proxied, the broken behavior would set it to unproxied).
- Fixed root error not being printed
- Add minor debug logging before cloudflare DNS update is successful

## 0.3.0 - 2020-05-30

- Add alternative WAN IP resolvers. Previously OpenDNS was used exclusively, but there exist networks where OpenDNS is not accessible. Now dness can issue HTTP requests instead of DNS to determine the WAN IP. See the readme for how to configure.
- The default deb packages now leverage an OpenSSL that has been statically compiled into the executable. While I believe that the ideal solution would be to distribute a pure dynamically linked application (libc + ssl) and a statically linked one, changes in creating a dynamically linked openssl application has made this ideal a bit more difficult to accomplish. Since my preference is statically linked executable anyways, I was ok with making the default deb package dynamically link libc but statically link openssl. If this is an issue, open an issue so that this can be investigated further.

## 0.2.1 - 2020-01-14

- Fixed an issue with the static builds not being deployed to github issues. No code changes.

## 0.2.0 - 2020-01-13

- Slight change to log entries
- Musl (static builds) releases now bind to rustls instead of openssl
- Add Namecheap provider
- Internal dependencies updated

## 0.1.1 - 2019-01-18

- Add GoDaddy provider
- Bump serde_json from 1.0.34 to 1.0.36
- Bump reqwest from 0.9.7 to 0.9.8

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
