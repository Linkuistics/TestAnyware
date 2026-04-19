import Testing
import CoreGraphics
@testable import TestAnywareAgent
import TestAnywareAgentProtocol

// MARK: - Helpers

private func makeElement(
    role: UnifiedRole,
    label: String? = nil,
    id: String? = nil,
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
        position: nil,
        size: nil,
        childCount: children?.count ?? 0,
        actions: [],
        platformRole: nil,
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
