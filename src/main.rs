extern crate trust_dns_resolver;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use std::net::*;
use trust_dns_resolver::Resolver;
use trust_dns_resolver::config::*;

// https://unix.stackexchange.com/questions/22615/how-can-i-get-my-external-ip-address-in-a-shell-script

#[derive(Serialize)]
struct CloudflareUpdate<'a> {
    type_: &'static str,
    name: &'a str,
    content: &'a str,
}

fn main() {
    let mut config = ResolverConfig::from_parts(
        None,
        vec![],
        NameServerConfigGroup::from_ips_clear(
                    &[
                        IpAddr::V4(Ipv4Addr::new(208, 67, 222, 222)),
                        IpAddr::V4(Ipv4Addr::new(208, 67, 220, 220)),
                    ], 53
    ));
    let mut resolver = Resolver::new(config, ResolverOpts::default()).unwrap();
    let mut response = resolver.ipv4_lookup("myip.opendns.com.").unwrap();
    let address = response.iter().next().expect("no addresses returned!");
    let addr: ::std::net:Ipv4Addr = address;
    println!("response: {:?}", response);

    let update = CloudflareUpdate {
        type_: "A",
        name: "nickbabcock.me",
        content: addr.to_string(),
    };

    // "https://api.cloudflare.com/client/v4/zones?name=example.com"

    let client = reqwest::Client::new();
    let res = client.put("https://api.cloudflare.com/client/v4/zones/<id>/dns_records/<id>")
        .header("X-Auth-Email", "me@example.com")
        .header("X-Auth-Key", "deadbeef")
        .json(&update)
        .send()
        .unwrap().status();


    println!("body = {}", res);
}
