import Testing
import CoreGraphics
@testable import TestAnywareAgent
import TestAnywareAgentProtocol

// MARK: - Helpers

private func makeInfo(
    role: UnifiedRole,
    label: String? = nil,
    id: String? = nil,
    position: CGPoint? = nil,
    size: CGSize? = nil,
    platformRole: String? = nil
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
        childCount: 0,
        actions: [],
        platformRole: platformRole,
        children: nil
    )
}

// MARK: - Identity match: positive cases

@Test func matchesWhenAllAttributesAlign() {
    let info = makeInfo(
        role: .textfield, label: nil, id: nil,
        position: CGPoint(x: 10, y: 20), size: CGSize(width: 100, height: 24),
        platformRole: "AXTextField"
    )
    let element = MockAccessibleElement(
        role: "AXTextField",
        position: CGPoint(x: 10, y: 20), size: CGSize(width: 100, height: 24)
    )
    #expect(LiveElementMatcher.matches(element, info: info))
}

@Test func matchesElementWithIdAndPlatformRole() {
    let info = makeInfo(
        role: .button, label: "Save", id: "saveBtn",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24),
        platformRole: "AXButton"
    )
    let element = MockAccessibleElement(
        role: "AXButton", label: "Save", identifier: "saveBtn",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    #expect(LiveElementMatcher.matches(element, info: info))
}

// MARK: - Identity match: regression for the bug

// The motivating bug: NSStackView exposes anonymous text fields where role
// matches across siblings. Snapshot dedup keeps both because position+size
// differ. The previous live walker matched only role+label+(maybe-pos),
// so --index 2 silently targeted the wrong element. With size in the
// match key, the walker correctly distinguishes them.
@Test func distinguishesStackViewSiblingsByPositionAndSize() {
    let firstInfo = makeInfo(
        role: .textfield, label: nil, id: nil,
        position: CGPoint(x: 10, y: 50), size: CGSize(width: 200, height: 22),
        platformRole: "AXTextField"
    )
    let secondInfo = makeInfo(
        role: .textfield, label: nil, id: nil,
        position: CGPoint(x: 10, y: 80), size: CGSize(width: 200, height: 22),
        platformRole: "AXTextField"
    )
    let firstLive = MockAccessibleElement(
        role: "AXTextField",
        position: CGPoint(x: 10, y: 50), size: CGSize(width: 200, height: 22)
    )
    let secondLive = MockAccessibleElement(
        role: "AXTextField",
        position: CGPoint(x: 10, y: 80), size: CGSize(width: 200, height: 22)
    )
    let stackView = MockAccessibleElement(role: "AXGroup", children: [firstLive, secondLive])

    #expect(LiveElementMatcher.find(in: [stackView], matching: firstInfo) != nil)
    #expect(LiveElementMatcher.find(in: [stackView], matching: secondInfo) != nil)

    // Distinct snapshots find distinct live elements.
    let foundFirst = LiveElementMatcher.find(in: [stackView], matching: firstInfo)
    let foundSecond = LiveElementMatcher.find(in: [stackView], matching: secondInfo)
    #expect(foundFirst?.position()?.y == 50)
    #expect(foundSecond?.position()?.y == 80)
}

// MARK: - Identity match: negative cases

@Test func rejectsWhenRoleDiffers() {
    let info = makeInfo(role: .button, label: "OK")
    let element = MockAccessibleElement(role: "AXTextField", label: "OK")
    #expect(!LiveElementMatcher.matches(element, info: info))
}

@Test func rejectsWhenLabelDiffers() {
    let info = makeInfo(role: .button, label: "Save")
    let element = MockAccessibleElement(role: "AXButton", label: "Cancel")
    #expect(!LiveElementMatcher.matches(element, info: info))
}

// nil-on-info means "snapshot recorded no label". A live element with a
// label is NOT the same identity — it's a sibling that happens to share
// role. Without this constraint, the walker would falsely match the
// first labelled sibling.
@Test func rejectsLabeledLiveElementWhenInfoLabelIsNil() {
    let info = makeInfo(role: .textfield, label: nil)
    let element = MockAccessibleElement(role: "AXTextField", label: "Username")
    #expect(!LiveElementMatcher.matches(element, info: info))
}

@Test func rejectsWhenIdDiffers() {
    let info = makeInfo(role: .button, label: "Save", id: "saveBtn")
    let element = MockAccessibleElement(role: "AXButton", label: "Save", identifier: "submitBtn")
    #expect(!LiveElementMatcher.matches(element, info: info))
}

@Test func rejectsLiveElementWithIdWhenInfoIdIsNil() {
    let info = makeInfo(role: .button, label: "Save", id: nil)
    let element = MockAccessibleElement(role: "AXButton", label: "Save", identifier: "saveBtn")
    #expect(!LiveElementMatcher.matches(element, info: info))
}

@Test func rejectsWhenPlatformRoleDiffers() {
    let info = makeInfo(
        role: .editableText, label: "Body",
        position: CGPoint(x: 0, y: 0), size: CGSize(width: 100, height: 24),
        platformRole: "AXTextArea"
    )
    let element = MockAccessibleElement(
        role: "AXTextField", label: "Body",
        position: CGPoint(x: 0, y: 0), size: CGSize(width: 100, height: 24)
    )
    #expect(!LiveElementMatcher.matches(element, info: info))
}

@Test func rejectsWhenPositionDiffers() {
    let info = makeInfo(
        role: .button, label: "Save",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    let element = MockAccessibleElement(
        role: "AXButton", label: "Save",
        position: CGPoint(x: 200, y: 10), size: CGSize(width: 80, height: 24)
    )
    #expect(!LiveElementMatcher.matches(element, info: info))
}

@Test func rejectsWhenSizeDiffers() {
    let info = makeInfo(
        role: .button, label: "Save",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    let element = MockAccessibleElement(
        role: "AXButton", label: "Save",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 200, height: 24)
    )
    #expect(!LiveElementMatcher.matches(element, info: info))
}

// MARK: - When info has no position/size

// Snapshot dedup skips elements without position+size (it can't tell two
// genuine duplicates from two off-screen siblings apart). When info has
// no position, the live matcher must not constrain on position either —
// otherwise it would gratuitously fail to find the element.
@Test func skipsPositionConstraintWhenInfoPositionIsNil() {
    let info = makeInfo(role: .button, label: "OK", id: "okBtn")
    let element = MockAccessibleElement(
        role: "AXButton", label: "OK", identifier: "okBtn",
        position: CGPoint(x: 999, y: 999), size: CGSize(width: 1, height: 1)
    )
    #expect(LiveElementMatcher.matches(element, info: info))
}

// MARK: - Tree traversal

@Test func findRecursesIntoChildren() {
    let info = makeInfo(
        role: .button, label: "Deep",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    let target = MockAccessibleElement(
        role: "AXButton", label: "Deep",
        position: CGPoint(x: 10, y: 10), size: CGSize(width: 80, height: 24)
    )
    let group = MockAccessibleElement(role: "AXGroup", children: [target])
    let window = MockAccessibleElement(role: "AXWindow", children: [group])

    let found = LiveElementMatcher.find(in: [window], matching: info)
    #expect(found != nil)
    #expect(found?.label() == "Deep")
}

@Test func findReturnsNilWhenNoElementMatches() {
    let info = makeInfo(role: .button, label: "Missing")
    let window = MockAccessibleElement(
        role: "AXWindow",
        children: [MockAccessibleElement(role: "AXButton", label: "Present")]
    )
    #expect(LiveElementMatcher.find(in: [window], matching: info) == nil)
}

@Test func findReturnsNilOnEmptyRoots() {
    let info = makeInfo(role: .button, label: "Anything")
    #expect(LiveElementMatcher.find(in: [], matching: info) == nil)
}
