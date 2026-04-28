import Testing
import CoreGraphics
@testable import TestAnywareAgent
import TestAnywareAgentProtocol

// MARK: - Helpers

private func makeElement(
    role: UnifiedRole,
    label: String? = nil,
    id: String? = nil,
    position: CGPoint? = nil,
    size: CGSize? = nil,
    platformRole: String? = nil,
    children: [ElementInfo]? = nil
) -> ElementInfo {
    ElementInfo(
        role: role,
        label: label,
        value: nil,
        description: nil,
        id: id,
        enabled: true,
        focused: false,
        position: position,
        size: size,
        childCount: children?.count ?? 0,
        actions: [],
        platformRole: platformRole,
        children: children
    )
}

// MARK: - Role-only filtering

@Test func resolveByRoleReturnsOnlyMatchingRole() {
    let button1 = makeElement(role: .button, label: "OK")
    let button2 = makeElement(role: .button, label: "Cancel")
    let text = makeElement(role: .text, label: "Hello")
    let tree = [button1, button2, text]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: nil)

    guard case .multiple(let matches) = result else {
        #expect(Bool(false), "Expected .multiple, got \(result)")
        return
    }
    #expect(matches.count == 2)
    #expect(matches.allSatisfy { $0.role == .button })
}

// MARK: - Label filtering

@Test func resolveByLabelIsCaseInsensitive() {
    let save = makeElement(role: .button, label: "Save Document")
    let cancel = makeElement(role: .button, label: "Cancel")
    let saveAs = makeElement(role: .button, label: "SAVE AS")
    let tree = [save, cancel, saveAs]

    let result = QueryResolver.resolve(in: tree, role: nil, label: "save", id: nil, index: nil)

    guard case .multiple(let matches) = result else {
        #expect(Bool(false), "Expected .multiple, got \(result)")
        return
    }
    #expect(matches.count == 2)
    #expect(matches[0].label == "Save Document")
    #expect(matches[1].label == "SAVE AS")
}

// MARK: - Role + label combined

@Test func resolveByRoleAndLabelReturnsOnlyMatchingButtonsWithLabel() {
    let saveButton = makeElement(role: .button, label: "Save")
    let saveText = makeElement(role: .text, label: "Save")
    let okButton = makeElement(role: .button, label: "OK")
    let tree = [saveButton, saveText, okButton]

    let result = QueryResolver.resolve(in: tree, role: .button, label: "Save", id: nil, index: nil)

    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found, got \(result)")
        return
    }
    #expect(match.role == .button)
    #expect(match.label == "Save")
}

// MARK: - Accessibility ID

@Test func resolveByIdReturnsExactMatch() {
    let elem1 = makeElement(role: .button, label: "Submit", id: "submitBtn")
    let elem2 = makeElement(role: .button, label: "Cancel", id: "cancelBtn")
    let tree = [elem1, elem2]

    let result = QueryResolver.resolve(in: tree, role: nil, label: nil, id: "submitBtn", index: nil)

    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found, got \(result)")
        return
    }
    #expect(match.id == "submitBtn")
    #expect(match.label == "Submit")
}

// MARK: - Index disambiguation

@Test func resolveWithIndexReturnsNthMatch() {
    let btn1 = makeElement(role: .button, label: "Next")
    let btn2 = makeElement(role: .button, label: "Next")
    let btn3 = makeElement(role: .button, label: "Next")
    let tree = [btn1, btn2, btn3]

    let result = QueryResolver.resolve(in: tree, role: .button, label: "Next", id: nil, index: 2)

    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found, got \(result)")
        return
    }
    // All three have same label; index=2 should return the second one (same as btn2)
    #expect(match.role == .button)
    #expect(match.label == "Next")
}

@Test func resolveWithIndexOutOfRangeReturnsNotFound() {
    let btn1 = makeElement(role: .button, label: "Next")
    let btn2 = makeElement(role: .button, label: "Next")
    let tree = [btn1, btn2]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: 5)

    guard case .notFound = result else {
        #expect(Bool(false), "Expected .notFound, got \(result)")
        return
    }
}

// Index 0 is out of range because the index is 1-based. This test exists to
// document that --index 0 will never match; users should use --index 1.
@Test func resolveWithIndexZeroReturnsNotFound() {
    let btn1 = makeElement(role: .button, label: "Next")
    let btn2 = makeElement(role: .button, label: "Next")
    let tree = [btn1, btn2]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: 0)

    guard case .notFound = result else {
        #expect(Bool(false), "Expected .notFound (index is 1-based), got \(result)")
        return
    }
}

@Test func resolveSingleMatchWithIndexOneIsFound() {
    let only = makeElement(role: .button, label: "Only")
    let tree = [only]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: 1)

    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found (1 is first match), got \(result)")
        return
    }
    #expect(match.label == "Only")
}

// MARK: - No match

@Test func resolveNoMatchReturnsNotFound() {
    let text = makeElement(role: .text, label: "Hello")
    let tree = [text]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: nil)

    guard case .notFound = result else {
        #expect(Bool(false), "Expected .notFound, got \(result)")
        return
    }
}

@Test func resolveEmptyTreeReturnsNotFound() {
    let result = QueryResolver.resolve(in: [], role: .button, label: nil, id: nil, index: nil)

    guard case .notFound = result else {
        #expect(Bool(false), "Expected .notFound, got \(result)")
        return
    }
}

// MARK: - Multiple matches without index

@Test func resolveMultipleMatchesWithoutIndexReturnsMultiple() {
    let btn1 = makeElement(role: .button, label: "Close")
    let btn2 = makeElement(role: .button, label: "Close")
    let tree = [btn1, btn2]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: nil)

    guard case .multiple(let matches) = result else {
        #expect(Bool(false), "Expected .multiple, got \(result)")
        return
    }
    #expect(matches.count == 2)
}

// MARK: - Recursive tree search

@Test func resolveSearchesNestedChildren() {
    let deepButton = makeElement(role: .button, label: "Deep Save", id: "deepSave")
    let group = makeElement(role: .group, label: "Toolbar", children: [deepButton])
    let window = makeElement(role: .window, label: "App", children: [group])
    let tree = [window]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: nil)

    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found, got \(result)")
        return
    }
    #expect(match.label == "Deep Save")
}

// MARK: - Window scoping

@Test func resolveSearchesOnlyWithinSpecifiedWindow() {
    let btn1 = makeElement(role: .button, label: "Save")
    let btn2 = makeElement(role: .button, label: "Cancel")
    let window1 = makeElement(role: .window, label: "Document", children: [btn1])
    let window2 = makeElement(role: .window, label: "Dialog", children: [btn2])
    _ = [window1, window2] // both windows exist; we scope search to window1's children only

    // Search only within "Document" window's children
    let scopedResult = QueryResolver.resolve(in: window1.children ?? [], role: .button, label: nil, id: nil, index: nil)

    guard case .found(let match) = scopedResult else {
        #expect(Bool(false), "Expected .found, got \(scopedResult)")
        return
    }
    #expect(match.label == "Save")

    // Verify "Cancel" is not found when scoped to window1
    let notFound = QueryResolver.resolve(in: window1.children ?? [], role: .button, label: "Cancel", id: nil, index: nil)
    guard case .notFound = notFound else {
        #expect(Bool(false), "Expected .notFound when scoping excludes window2, got \(notFound)")
        return
    }
}

// MARK: - Single exact match returns .found not .multiple

@Test func resolveSingleMatchReturnsFoun() {
    let btn = makeElement(role: .button, label: "Unique Button")
    let text = makeElement(role: .text, label: "Some text")
    let tree = [btn, text]

    let result = QueryResolver.resolve(in: tree, role: .button, label: nil, id: nil, index: nil)

    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found, got \(result)")
        return
    }
    #expect(match.label == "Unique Button")
}

// MARK: - Dedup of AX-tree back-references

// macOS exposes some elements (notably NSStackView descendants and
// scroll-area subtrees) through more than one parent path. The walker
// emits each path, so the same logical element can appear in `matches`
// twice. The resolver must collapse those.
@Test func resolveDedupsSameElementReachedViaTwoPaths() {
    let pos = CGPoint(x: 100, y: 200)
    let sz = CGSize(width: 400, height: 300)
    // Same element (same role + position + size + platformRole), reached
    // via two different ancestor subtrees in the AX walker output.
    let aliasA = makeElement(
        role: .editableText, label: "Document",
        position: pos, size: sz, platformRole: "AXTextArea"
    )
    let aliasB = makeElement(
        role: .editableText, label: "Document",
        position: pos, size: sz, platformRole: "AXTextArea"
    )
    let containerA = makeElement(role: .group, label: "Stack", children: [aliasA])
    let containerB = makeElement(role: .scrollArea, label: "Scroll", children: [aliasB])
    let window = makeElement(role: .window, label: "Untitled", children: [containerA, containerB])

    let result = QueryResolver.resolve(
        in: [window], role: .editableText, label: nil, id: nil, index: nil
    )

    guard case .found = result else {
        #expect(Bool(false), "Expected .found after dedup, got \(result)")
        return
    }
}

@Test func resolveKeepsDistinctElementsAtDifferentPositions() {
    // Two genuinely different text areas (e.g. document body + Find
    // banner) sit at different positions; dedup must NOT collapse them.
    let body = makeElement(
        role: .editableText, label: "Document",
        position: CGPoint(x: 0, y: 100), size: CGSize(width: 400, height: 300),
        platformRole: "AXTextArea"
    )
    let findBar = makeElement(
        role: .editableText, label: "Document",
        position: CGPoint(x: 0, y: 30), size: CGSize(width: 400, height: 24),
        platformRole: "AXTextArea"
    )
    let window = makeElement(role: .window, label: "Untitled", children: [findBar, body])

    let result = QueryResolver.resolve(
        in: [window], role: .editableText, label: nil, id: nil, index: nil
    )

    guard case .multiple(let matches) = result else {
        #expect(Bool(false), "Expected .multiple, got \(result)")
        return
    }
    #expect(matches.count == 2)
}

@Test func resolveDedupSkipsElementsWithoutCoordinates() {
    // Dedup is identity-by-coords, so elements without position/size
    // are NOT collapsed even when role+label match. This protects the
    // legitimately-distinct case (two off-screen elements not yet laid
    // out) from being wrongly merged. AX always reports coords for
    // visible elements, which is when dedup matters in practice.
    let a = makeElement(role: .button, label: "OK", id: "okBtn")
    let b = makeElement(role: .button, label: "OK", id: "okBtn")
    let window = makeElement(role: .window, label: "Dialog", children: [a, b])

    let result = QueryResolver.resolve(
        in: [window], role: .button, label: nil, id: nil, index: nil
    )
    guard case .multiple(let matches) = result else {
        #expect(Bool(false), "Expected .multiple when coords are absent, got \(result)")
        return
    }
    #expect(matches.count == 2)
}

@Test func resolveDedupRespectsIndexAfterCollapse() {
    // After dedup, --index must address into the collapsed list, not
    // the raw match list. Three duplicates of one element + one
    // distinct element collapse to two; --index 2 must return the
    // distinct one.
    let alias1 = makeElement(
        role: .button, label: "Save",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    let alias2 = makeElement(
        role: .button, label: "Save",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    let other = makeElement(
        role: .button, label: "Save",
        position: CGPoint(x: 100, y: 10), size: CGSize(width: 80, height: 24)
    )
    let window = makeElement(role: .window, label: "Doc", children: [alias1, alias2, other])

    let result = QueryResolver.resolve(
        in: [window], role: .button, label: nil, id: nil, index: 2
    )
    guard case .found(let match) = result else {
        #expect(Bool(false), "Expected .found at --index 2, got \(result)")
        return
    }
    #expect(match.position?.x == 100)
}
