//! Who may use the socket.

/// The app serves a connection only when its peer runs as the same user as the app.
/// A pure function, so a test can drive it with any uid.
pub fn peer_allowed(peer_uid: u32, own_uid: u32) -> bool {
    peer_uid == own_uid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_uid_is_allowed() {
        assert!(peer_allowed(501, 501));
    }

    #[test]
    fn any_other_uid_is_refused_including_root() {
        assert!(!peer_allowed(0, 501));
        assert!(!peer_allowed(502, 501));
    }
}
