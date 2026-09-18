//! Whether a peer's `X-Auth-*` headers are believed.
//!
//! The identity headers name the caller and carry the role, so they are
//! trustworthy only from the gateway that sets them. Every other peer reaches
//! Burst as an anonymous caller, whatever it sends.

use std::net::IpAddr;

/// Whether `peer` appears in a list of addresses and CIDR ranges.
pub fn is_trusted(peer: IpAddr, trusted: &[String]) -> bool {
    let peer = normalize(peer);
    trusted.iter().any(|entry| matches(peer, entry))
}

/// Whether an entry parses as an address or a CIDR range.
///
/// Used at startup so a typo is a refusal to start rather than a peer that
/// silently never matches.
pub fn is_valid_entry(entry: &str) -> bool {
    match entry.split_once('/') {
        Some((network, prefix)) => {
            let Ok(network) = network.trim().parse::<IpAddr>() else {
                return false;
            };
            let Ok(prefix) = prefix.trim().parse::<u8>() else {
                return false;
            };
            prefix <= max_bits(network)
        }
        None => entry.trim().parse::<IpAddr>().is_ok(),
    }
}

/// An IPv4 address arriving over a dual-stack socket is mapped into IPv6
/// (`::ffff:127.0.0.1`). Comparing that against an IPv4 range would never
/// match, so it is unmapped first.
fn normalize(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        other => other,
    }
}

fn max_bits(ip: IpAddr) -> u8 {
    match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    }
}

fn matches(peer: IpAddr, entry: &str) -> bool {
    match entry.split_once('/') {
        Some((network, prefix)) => match (
            network.trim().parse::<IpAddr>(),
            prefix.trim().parse::<u8>(),
        ) {
            (Ok(network), Ok(prefix)) => in_cidr(peer, network, prefix),
            _ => false,
        },
        None => entry
            .trim()
            .parse::<IpAddr>()
            .map(|e| normalize(e) == peer)
            .unwrap_or(false),
    }
}

fn in_cidr(ip: IpAddr, network: IpAddr, prefix_len: u8) -> bool {
    match (ip, normalize(network)) {
        (IpAddr::V4(ip), IpAddr::V4(net)) => {
            bits_match(&ip.octets(), &net.octets(), prefix_len, 32)
        }
        (IpAddr::V6(ip), IpAddr::V6(net)) => {
            bits_match(&ip.octets(), &net.octets(), prefix_len, 128)
        }
        // A range of one family never matches an address of the other.
        _ => false,
    }
}

/// Compare the leading `prefix_len` bits of two addresses.
fn bits_match(a: &[u8], b: &[u8], prefix_len: u8, max_bits: u8) -> bool {
    if prefix_len > max_bits {
        return false;
    }
    let mut remaining = prefix_len as usize;
    for (x, y) in a.iter().zip(b.iter()) {
        if remaining == 0 {
            break;
        }
        let take = remaining.min(8);
        let mask: u8 = if take == 8 { 0xFF } else { !0u8 << (8 - take) };
        if (x & mask) != (y & mask) {
            return false;
        }
        remaining -= take;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().expect("address")
    }

    #[test]
    fn a_single_address_matches_only_itself() {
        let list = vec!["10.0.0.5".to_string()];
        assert!(is_trusted(ip("10.0.0.5"), &list));
        assert!(!is_trusted(ip("10.0.0.6"), &list));
    }

    #[test]
    fn a_range_matches_its_members() {
        let list = vec!["10.0.0.0/8".to_string()];
        assert!(is_trusted(ip("10.255.255.254"), &list));
        assert!(!is_trusted(ip("11.0.0.1"), &list));
    }

    /// A dual-stack listener reports an IPv4 peer as an IPv4-mapped IPv6
    /// address, which would otherwise never match an IPv4 range and would
    /// leave the gateway itself untrusted.
    #[test]
    fn a_mapped_ipv4_peer_matches_an_ipv4_range() {
        assert!(is_trusted(
            ip("::ffff:127.0.0.1"),
            &["127.0.0.1/32".to_string()]
        ));
        assert!(is_trusted(
            ip("::ffff:10.1.2.3"),
            &["10.0.0.0/8".to_string()]
        ));
    }

    #[test]
    fn families_do_not_cross() {
        assert!(!is_trusted(ip("10.0.0.1"), &["::1/128".to_string()]));
        assert!(!is_trusted(ip("::1"), &["10.0.0.0/8".to_string()]));
    }

    #[test]
    fn ipv6_ranges_work() {
        let list = vec!["2001:db8::/32".to_string()];
        assert!(is_trusted(ip("2001:db8::1"), &list));
        assert!(!is_trusted(ip("2001:db9::1"), &list));
    }

    #[test]
    fn an_empty_list_trusts_nobody() {
        assert!(!is_trusted(ip("127.0.0.1"), &[]));
    }

    #[test]
    fn a_malformed_entry_never_matches_and_is_refused_at_startup() {
        assert!(!is_trusted(ip("127.0.0.1"), &["not-an-ip".to_string()]));
        assert!(!is_valid_entry("not-an-ip"));
        assert!(!is_valid_entry("10.0.0.0/33"));
        assert!(!is_valid_entry("10.0.0.0/abc"));
        assert!(is_valid_entry("10.0.0.0/8"));
        assert!(is_valid_entry("127.0.0.1"));
        assert!(is_valid_entry("2001:db8::/32"));
    }
}
