import TestAnywareAgentProtocol

/// Locates a live `AccessibleElement` in a tree of root elements that
/// matches a snapshot `ElementInfo` produced by `TreeWalker`.
///
/// The match key mirrors `QueryResolver.identityKey` exactly — role,
/// label, id, platformRole, position, size — so that an element
/// resolved by `--index N` from a deduped snapshot list can be
/// relocated in the live AX tree even when role+label alone collide.
/// The motivating case: NSStackView exposes anonymous text fields
/// where role and label match across siblings; only position+size
/// distinguish them in dedup, so the live walker needs the same
/// signature or it will return the first sibling and the caller's
/// `--index 2` request silently targets the wrong field.
///
/// Invariant: this matcher's predicate must stay consistent with
/// `QueryResolver.identityKey`. If you add an attribute to one,
/// add it to the other.
public enum LiveElementMatcher {

    /// Depth-first search through `roots` and their descendants,
    /// returning the first live element whose identity matches `info`.
    public static func find(
        in roots: [any AccessibleElement],
        matching info: ElementInfo
    ) -> (any AccessibleElement)? {
        for root in roots {
            if matches(root, info: info) { return root }
            if let found = find(in: root.children(), matching: info) {
                return found
            }
        }
        return nil
    }

    /// True when `element`'s identity attributes match `info`.
    ///
    /// Each attribute is compared in the same nil-equivalence sense
    /// `dedupedByIdentity` uses: nil on the snapshot side means "the
    /// snapshot recorded no value", which constrains the live element
    /// to also have no value for that attribute (otherwise a sibling
    /// with the desired attribute would falsely match).
    ///
    /// Position and size, when present on `info`, require the live
    /// element to also report the same values — stricter than the
    /// previous "skip if either side is nil" behaviour, which let
    /// stack-view siblings collapse onto the first match.
    static func matches(_ element: any AccessibleElement, info: ElementInfo) -> Bool {
        let mappedRole = RoleMapper.map(role: element.role() ?? "", subrole: element.subrole())
        guard mappedRole == info.role else { return false }

        if info.label != element.label() { return false }
        if info.id != element.identifier() { return false }

        if let platformRole = info.platformRole {
            guard element.role() == platformRole else { return false }
        }

        if let pos = info.position {
            guard let elementPos = element.position(),
                  elementPos.x == pos.x, elementPos.y == pos.y
            else { return false }
        }
        if let sz = info.size {
            guard let elementSize = element.size(),
                  elementSize.width == sz.width, elementSize.height == sz.height
            else { return false }
        }
        return true
    }
}
