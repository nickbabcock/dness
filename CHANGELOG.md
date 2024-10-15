## 0.5.7 - 2024-10-15

- Update default porkbun API domain to `api.porkbun.com`
- Update internal dependencies to latest

## 0.5.6 - 2023-12-02

- Add support for porkbun domains
  ```toml
  [[domains]]
  # denote that the domain is managed by porkbun
  type = "porkbun"

  # The Porkbun domain: https://porkbun.com/account/domainsSpeedy
  # IMPORTANT: You must enable API Access for the domain at the above url.
  domain = "example.com"

  # This is the api key, you can create one here:
  # https://porkbun.com/account/api
  key = "abc123"

  # The password for the key, top secret! Only visible once when you create the key.
  secret = "ef"

  # The records to update. "@" = "example.com", "a" = "a.example.com" "*" = "*.example.com"
  # Both "@" and "" are valid to configure root domain.
  records = [ "@", "a" ]
  ```
- Dependency update

## 0.5.5 - 2022-01-06

v0.5.4 wasn't properly released, so v0.5.5 is v0.5.4.

## 0.5.4 - 2022-01-06

- Add m1 (aarch64) mac builds
- Update dependencies

## 0.5.3 - 2021-05-18

- Add support for dynu domains:

```toml
[[domains]]
type = "dynu"
hostname = "test-dness-1.xyz"
username = "MyUserName"

# ip update password:
# https://www.dynu.com/en-US/ControlPanel/ManageCredentials
password = "IpUpdatePassword"

# The records to update.
# "@" = "test-dness-1.xyz"
# "sub = "sub.test-dness-1.xyz"
records = [ "@", "sub" ]
```

## 0.5.2 - 2021-05-12

- Fixed deb packaging for dpkg >= 1.20.1 (ubuntu 21.04)
- Add support for no-ip domains:

```toml
[[domains]]
type = "noip"
hostname = "dnesstest.hopto.org"
username = "myemail@example.org"
password = "super_secret_password"
```

## 0.5.1 - 2021-04-02

Add support for [he.net](http://he.net/). Below is a sample config:

```toml
[[domains]]
type = "he"
hostname = "test-dness-1.xyz"
password = "super_secret_password"
records = [ "@", "sub" ]
```

## 0.5.0 - 2020-12-29

This release is for the sysadmins out there. The dness config file is now treated as a handlebar template with variables filled in from the environment. Now one can write

```toml
[[domains]]
type = "cloudflare"
token = "{{MY_CLOUDFLARE_TOKEN}}"
zone = "example.com"
records = [
    "n.example.com"
]
````

And if `MY_CLOUDFLARE_TOKEN` is in the environment then dness can be executed as an unprivileged, dynamic user. This mainly affects systemd users who will now want to extract sensitive info into:

```
/etc/dness/dness.env
```

and format it like so:

```
MY_CLOUDFLARE_TOKEN=dec0de
```

Also for systemd users, the provided service file now sandboxes dness properly.

This release also consolidates the x86 linux builds to only builds that are built with musl with openssl statically compiled. This should be a minor annoyance. Those that want everything dynamically linked are encouraged to build from source, and users of the musl deb variant (myself included) will need to migrate to the new deb with:

```
dpkg --remove dness-musl
dpkg --install dness_0.5.0_amd64.deb
```

## 0.4.0 - 2020-08-07

Add new `token` field to Cloudflare configs representing a Cloudflare API token. Using an API token is preferred to specifying email + key as a token can be tailored to the desired permissions. When creating a new token, the "Edit zone DNS" API token template in Cloudflare can be selected to simplify token setup.

To migrate take an old Cloudflare config:

```toml
[[domains]]
type = "cloudflare"
email = "admin@example.com"
key = "deadbeef"
zone = "example.com"
records = [
    "n.example.com"
]
```

And remove the `email` and `key` fields and replace with the appropriately permissioned `token`:

```toml
[[domains]]
type = "cloudflare"
token = "dec0de"
zone = "example.com"
records = [
    "n.example.com"
]
```

Email + key is will still be supported, but using the `token` field is now preferred.

Big thanks to *@luckyrat* who spearheaded this effort.

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
