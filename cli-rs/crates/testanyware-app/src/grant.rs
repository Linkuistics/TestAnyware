//! The app's last observation of the Local Network grant.
//!
//! Only a dial to a network that one of the host's interfaces attaches, other than loopback,
//! says anything about the grant. A dial to loopback, or to an address beyond the attached
//! networks, leaves the observation as it was (spec: "The Local Network diagnosis").

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::SystemTime;

/// What the app last learned about the grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantObservation {
    /// No dial of the kind that tells has happened yet.
    NotYetSeen,
    /// A dial to an attached network connected.
    Allowed {
        destination: SocketAddr,
        at: SystemTime,
    },
    /// A dial to an attached network failed, and the platform `nc` proved the denial.
    Denied {
        destination: SocketAddr,
        at: SystemTime,
    },
}

/// What a dial showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialOutcome {
    Connected,
    /// The platform tool connects where the app could not.
    DeniedProved,
    /// Any other end: a refusal, a timeout, or an unreachable host that `nc` cannot reach.
    Neither,
}

/// A network that a host interface attaches: the interface's address and its prefix length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostNetwork {
    pub address: IpAddr,
    pub prefix_len: u8,
}

/// The observation after a dial. `networks` lists the networks of the non-loopback
/// interfaces that are up.
pub fn observe(
    previous: &GrantObservation,
    destination: SocketAddr,
    outcome: DialOutcome,
    networks: &[HostNetwork],
    now: SystemTime,
) -> GrantObservation {
    if !is_attached_network(destination.ip(), networks) {
        return previous.clone();
    }
    match outcome {
        DialOutcome::Connected => GrantObservation::Allowed {
            destination,
            at: now,
        },
        DialOutcome::DeniedProved => GrantObservation::Denied {
            destination,
            at: now,
        },
        DialOutcome::Neither => previous.clone(),
    }
}

/// Does `ip` sit in a network that a host interface attaches, and is it neither loopback nor
/// an address of the host itself? A dial to an own address never reaches the network, so it
/// shows nothing about the grant.
pub fn is_attached_network(ip: IpAddr, networks: &[HostNetwork]) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return false;
    }
    if networks.iter().any(|n| n.address == ip) {
        return false;
    }
    networks.iter().any(|n| same_network(ip, n))
}

fn same_network(ip: IpAddr, network: &HostNetwork) -> bool {
    match (ip, network.address) {
        (IpAddr::V4(a), IpAddr::V4(b)) => {
            let mask = prefix_mask_v4(network.prefix_len);
            u32::from(a) & mask == u32::from(b) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(b)) => {
            let mask = prefix_mask_v6(network.prefix_len);
            u128::from(a) & mask == u128::from(b) & mask
        }
        _ => false,
    }
}

fn prefix_mask_v4(len: u8) -> u32 {
    match len.min(32) {
        0 => 0,
        n => u32::MAX << (32 - n),
    }
}

fn prefix_mask_v6(len: u8) -> u128 {
    match len.min(128) {
        0 => 0,
        n => u128::MAX << (128 - n),
    }
}

/// The prefix length of a netmask: the number of leading one bits.
pub fn prefix_len_v4(mask: Ipv4Addr) -> u8 {
    u32::from(mask).leading_ones() as u8
}

pub fn prefix_len_v6(mask: Ipv6Addr) -> u8 {
    u128::from(mask).leading_ones() as u8
}

/// The networks of the host's interfaces that are up and not loopback.
#[cfg(unix)]
pub fn host_networks() -> Vec<HostNetwork> {
    use nix::net::if_::InterfaceFlags;

    // https://docs.rs/nix/0.29.0/nix/ifaddrs/fn.getifaddrs.html needs nix's `net` feature.
    let Ok(interfaces) = nix::ifaddrs::getifaddrs() else {
        // Without the list, no dial counts as attached, and the observation stays as it was.
        return Vec::new();
    };
    interfaces
        .filter(|i| {
            i.flags.contains(InterfaceFlags::IFF_UP)
                && !i.flags.contains(InterfaceFlags::IFF_LOOPBACK)
        })
        .filter_map(|i| {
            let (address, netmask) = (i.address?, i.netmask?);
            if let (Some(a), Some(m)) = (address.as_sockaddr_in(), netmask.as_sockaddr_in()) {
                return Some(HostNetwork {
                    address: IpAddr::V4(a.ip()),
                    prefix_len: prefix_len_v4(m.ip()),
                });
            }
            let (a, m) = (address.as_sockaddr_in6()?, netmask.as_sockaddr_in6()?);
            Some(HostNetwork {
                address: IpAddr::V6(a.ip()),
                prefix_len: prefix_len_v6(m.ip()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn net(address: IpAddr, prefix_len: u8) -> HostNetwork {
        HostNetwork {
            address,
            prefix_len,
        }
    }

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn dest(ip: IpAddr) -> SocketAddr {
        SocketAddr::new(ip, 8648)
    }

    /// The host: a LAN interface and the tart bridge.
    fn host() -> Vec<HostNetwork> {
        vec![net(v4(192, 168, 1, 20), 24), net(v4(192, 168, 64, 1), 24)]
    }

    #[test]
    fn the_tart_guest_subnet_is_an_attached_network() {
        assert!(is_attached_network(v4(192, 168, 64, 5), &host()));
        assert!(is_attached_network(v4(192, 168, 1, 99), &host()));
    }

    #[test]
    fn loopback_is_never_attached() {
        // Even when the list wrongly holds a loopback interface.
        let networks = vec![net(v4(127, 0, 0, 1), 8)];
        assert!(!is_attached_network(v4(127, 0, 0, 1), &networks));
        assert!(!is_attached_network(v4(127, 0, 0, 2), &networks));
        assert!(!is_attached_network(
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            &host()
        ));
    }

    #[test]
    fn an_address_beyond_the_attached_networks_is_not_attached() {
        assert!(!is_attached_network(v4(10, 0, 0, 5), &host()));
        assert!(!is_attached_network(v4(192, 168, 65, 5), &host()));
        assert!(!is_attached_network(v4(8, 8, 8, 8), &host()));
    }

    #[test]
    fn an_address_of_the_host_itself_is_not_attached() {
        assert!(!is_attached_network(v4(192, 168, 64, 1), &host()));
        assert!(!is_attached_network(v4(192, 168, 1, 20), &host()));
    }

    #[test]
    fn the_prefix_length_decides_the_network() {
        let wide = vec![net(v4(10, 1, 2, 3), 8)];
        assert!(is_attached_network(v4(10, 200, 0, 9), &wide));
        let narrow = vec![net(v4(10, 1, 2, 3), 24)];
        assert!(!is_attached_network(v4(10, 200, 0, 9), &narrow));
        let none = vec![net(v4(10, 1, 2, 3), 0)];
        assert!(
            is_attached_network(v4(172, 16, 0, 1), &none),
            "/0 attaches every address"
        );
    }

    #[test]
    fn an_ipv6_network_matches_by_prefix_and_never_matches_an_ipv4_address() {
        let ula: Ipv6Addr = "fd00:1::2".parse().unwrap();
        let networks = vec![net(IpAddr::V6(ula), 64)];
        assert!(is_attached_network(
            "fd00:1::99".parse().unwrap(),
            &networks
        ));
        assert!(!is_attached_network(
            "fd00:2::99".parse().unwrap(),
            &networks
        ));
        assert!(!is_attached_network(v4(192, 168, 64, 5), &networks));
    }

    #[test]
    fn netmasks_give_their_prefix_length() {
        assert_eq!(prefix_len_v4(Ipv4Addr::new(255, 255, 255, 0)), 24);
        assert_eq!(prefix_len_v4(Ipv4Addr::new(255, 255, 252, 0)), 22);
        assert_eq!(prefix_len_v4(Ipv4Addr::new(0, 0, 0, 0)), 0);
        assert_eq!(prefix_len_v6("ffff:ffff:ffff:ffff::".parse().unwrap()), 64);
    }

    #[test]
    fn a_connected_dial_to_an_attached_network_records_allowed() {
        let guest = dest(v4(192, 168, 64, 5));
        let seen = observe(
            &GrantObservation::NotYetSeen,
            guest,
            DialOutcome::Connected,
            &host(),
            at(10),
        );
        assert_eq!(
            seen,
            GrantObservation::Allowed {
                destination: guest,
                at: at(10)
            }
        );
    }

    #[test]
    fn a_proved_denial_records_denied() {
        let guest = dest(v4(192, 168, 64, 5));
        let allowed = GrantObservation::Allowed {
            destination: guest,
            at: at(10),
        };
        let seen = observe(&allowed, guest, DialOutcome::DeniedProved, &host(), at(20));
        assert_eq!(
            seen,
            GrantObservation::Denied {
                destination: guest,
                at: at(20)
            }
        );
    }

    #[test]
    fn a_later_connection_replaces_a_denial() {
        let guest = dest(v4(192, 168, 64, 5));
        let denied = GrantObservation::Denied {
            destination: guest,
            at: at(20),
        };
        let seen = observe(&denied, guest, DialOutcome::Connected, &host(), at(30));
        assert_eq!(
            seen,
            GrantObservation::Allowed {
                destination: guest,
                at: at(30)
            }
        );
    }

    #[test]
    fn a_dial_to_loopback_leaves_every_observation_as_it_was() {
        let guest = dest(v4(192, 168, 64, 5));
        for previous in [
            GrantObservation::NotYetSeen,
            GrantObservation::Allowed {
                destination: guest,
                at: at(1),
            },
            GrantObservation::Denied {
                destination: guest,
                at: at(1),
            },
        ] {
            for outcome in [
                DialOutcome::Connected,
                DialOutcome::DeniedProved,
                DialOutcome::Neither,
            ] {
                let loopback = dest(v4(127, 0, 0, 1));
                assert_eq!(
                    observe(&previous, loopback, outcome, &host(), at(9)),
                    previous
                );
            }
        }
    }

    #[test]
    fn a_dial_beyond_the_attached_networks_leaves_the_observation_as_it_was() {
        let guest = dest(v4(192, 168, 64, 5));
        let previous = GrantObservation::Denied {
            destination: guest,
            at: at(1),
        };
        let far = dest(v4(10, 9, 9, 9));
        assert_eq!(
            observe(&previous, far, DialOutcome::Connected, &host(), at(9)),
            previous
        );
    }

    #[test]
    fn a_failure_that_proves_nothing_leaves_the_observation_as_it_was() {
        let guest = dest(v4(192, 168, 64, 5));
        let previous = GrantObservation::Allowed {
            destination: guest,
            at: at(1),
        };
        assert_eq!(
            observe(&previous, guest, DialOutcome::Neither, &host(), at(9)),
            previous
        );
    }

    #[test]
    fn the_real_interface_list_holds_no_loopback_address() {
        for network in host_networks() {
            assert!(!network.address.is_loopback(), "{network:?}");
        }
    }
}
