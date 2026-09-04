//!
//! This code is taken from:
//! `https://github.com/sigoden/dufs/blob/main/src/main.rs`
//! Its under Apache License & belongs to respective owners!!
//! `https://spdx.org/licenses/Apache-2.0.html`
//! Credit: @sigoden
//!
use std::net::IpAddr;
use anyhow::{Context, Result};

use crate::config::AppConf;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindAddr {
    IpAddr(IpAddr),
}

pub fn interface_addrs() -> Result<(Vec<BindAddr>, Vec<BindAddr>)> {
    let (mut ipv4_addrs, mut ipv6_addrs) = (vec![], vec![]);
    let ifaces = if_addrs::get_if_addrs().with_context(|| "Failed to get local interface addresses")?;
    for iface in ifaces.into_iter() {
        let ip = iface.ip();
        if ip.is_ipv4() {
            ipv4_addrs.push(BindAddr::IpAddr(ip))
        }
        if ip.is_ipv6() {
            ipv6_addrs.push(BindAddr::IpAddr(ip))
        }
    }
    Ok((ipv4_addrs, ipv6_addrs))
}

pub fn print_listening(conf: &AppConf, print_addrs: &[BindAddr]) -> Result<String> {
    let mut output = String::new();
    let urls = print_addrs
        .iter()
        .map(|bind_addr| match bind_addr {
            BindAddr::IpAddr(addr) => {
                let addr = match addr {
                    IpAddr::V4(_) => format!("{}:{}", addr, conf.port),
                    IpAddr::V6(_) => format!("[{}]:{}", addr, conf.port),
                };
				let has_cert = conf.ssl_cert.as_ref().map_or(false, |p| !p.as_os_str().is_empty());
				let has_key = conf.ssl_key.as_ref().map_or(false, |p| !p.as_os_str().is_empty());
                let protocol = if has_cert && has_key {
                    "https"
                } else {
                    "http"
                };
                format!("{}://{}", protocol, addr)
            }
        })
        .collect::<Vec<_>>();

    if urls.len() == 1 {
        output.push_str(&format!("Qateel listening on: {}", urls[0]))
    } else {
        let info = urls
            .iter()
            .map(|v| format!("  {v}"))
            .collect::<Vec<String>>()
            .join("\n\t\t\t\t\t");
        output.push_str(&format!("Qateel listening on:\n\n\t\t\t\t\t{info}\n"))
    }

    Ok(output)
}

