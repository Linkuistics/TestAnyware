import Testing
import CoreGraphics
import TestAnywareAgent
import TestAnywareAgentProtocol

// MARK: - Full-depth walking

@Test func walkThreeLevelTreeReturnsAllElements() {
    let leaf1 = MockAccessibleElement(role: "AXStaticText", label: "Leaf1")
    let leaf2 = MockAccessibleElement(role: "AXStaticText", label: "Leaf2")
    let mid = MockAccessibleElement(role: "AXGroup", label: "Mid", children: [leaf1, leaf2])
    let root = MockAccessibleElement(role: "AXWindow", label: "Root", children: [mid])

    let result = TreeWalker.walk(root: root, depth: 3)

    // Root has 1 child (mid), mid has 2 children (leaf1, leaf2)
    #expect(result.count == 1) // walk returns root's children
    let midInfo = result[0]
    #expect(midInfo.role == .group)
    #expect(midInfo.label == "Mid")
    #expect(midInfo.children?.count == 2)
    #expect(midInfo.children?[0].role == .text)
    #expect(midInfo.children?[0].label == "Leaf1")
    #expect(midInfo.children?[1].label == "Leaf2")
}

// MARK: - Depth limiting

@Test func walkDepthOneReturnsOnlyTopLevelChildren() {
    let leaf = MockAccessibleElement(role: "AXStaticText", label: "Leaf")
    let mid = MockAccessibleElement(role: "AXGroup", label: "Mid", children: [leaf])
    let root = MockAccessibleElement(role: "AXWindow", label: "Root", children: [mid])

    let result = TreeWalker.walk(root: root, depth: 1)

    #expect(result.count == 1)
    let midInfo = result[0]
    #expect(midInfo.label == "Mid")
    // Children should be nil (depth exhausted) but childCount reflects actual count
    #expect(midInfo.children == nil)
    #expect(midInfo.childCount == 1)
}

// MARK: - Role filtering

@Test func walkFilterByRoleIncludesOnlyMatchingRole() {
    let button1 = MockAccessibleElement(role: "AXButton", label: "OK")
    let text1 = MockAccessibleElement(role: "AXStaticText", label: "Hello")
    let button2 = MockAccessibleElement(role: "AXButton", label: "Cancel")
    let group = MockAccessibleElement(role: "AXGroup", label: "Container", children: [button1, text1, button2])
    let root = MockAccessibleElement(role: "AXWindow", label: "Win", children: [group])

    let result = TreeWalker.walk(root: root, depth: 3, roleFilter: .button)

    // The group should still appear because it has matching descendants
    #expect(result.count == 1)
    let groupInfo = result[0]
    #expect(groupInfo.role == .group)
    // Only buttons should be in children
    #expect(groupInfo.children?.count == 2)
    #expect(groupInfo.children?[0].label == "OK")
    #expect(groupInfo.children?[1].label == "Cancel")
}

// MARK: - Label filtering

@Test func walkFilterByLabelCaseInsensitive() {
    let elem1 = MockAccessibleElement(role: "AXButton", label: "Save Document")
    let elem2 = MockAccessibleElement(role: "AXButton", label: "Cancel")
    let elem3 = MockAccessibleElement(role: "AXButton", label: "save as")
    let root = MockAccessibleElement(role: "AXWindow", label: "Win", children: [elem1, elem2, elem3])

    let result = TreeWalker.walk(root: root, depth: 2, labelFilter: "save")

    // Only elements with "save" (case-insensitive) in their label
    #expect(result.count == 2)
    #expect(result[0].label == "Save Document")
    #expect(result[1].label == "save as")
}

// MARK: - Element conversion

@Test func walkConvertsAccessibleElementToElementInfoCorrectly() {
    let element = MockAccessibleElement(
        role: "AXButton",
        subrole: nil,
        label: "OK",
        value: "1",
        description: "Confirm button",
        identifier: "btn-ok",
        enabled: true,
        focused: true,
        position: CGPoint(x: 10, y: 20),
        size: CGSize(width: 80, height: 30),
        children: [],
        actionNames: ["AXPress"]
    )
    let root = MockAccessibleElement(role: "AXWindow", children: [element])

    let result = TreeWalker.walk(root: root, depth: 2)

    #expect(result.count == 1)
    let info = result[0]
    #expect(info.role == .button) // mapped via RoleMapper
    #expect(info.label == "OK")
    #expect(info.value == "1")
    #expect(info.description == "Confirm button")
    #expect(info.id == "btn-ok")
    #expect(info.enabled == true)
    #expect(info.focused == true)
    #expect(info.position == CGPoint(x: 10, y: 20))
    #expect(info.size == CGSize(width: 80, height: 30))
    #expect(info.actions == ["AXPress"])
    #expect(info.platformRole == "AXButton")
    #expect(info.childCount == 0)
    #expect(info.children?.count == 0)
}

// MARK: - Child count at depth limit

@Test func walkChildCountSetCorrectlyWhenDepthLimitReached() {
    let leaf1 = MockAccessibleElement(role: "AXStaticText", label: "A")
    let leaf2 = MockAccessibleElement(role: "AXStaticText", label: "B")
    let leaf3 = MockAccessibleElement(role: "AXStaticText", label: "C")
    let mid = MockAccessibleElement(role: "AXGroup", label: "Group", children: [leaf1, leaf2, leaf3])
    let root = MockAccessibleElement(role: "AXWindow", children: [mid])

    let result = TreeWalker.walk(root: root, depth: 1)

    #expect(result.count == 1)
    let midInfo = result[0]
    #expect(midInfo.childCount == 3) // actual child count
    #expect(midInfo.children == nil) // not expanded due to depth limit
}

// MARK: - H4: Menu bar walking

@Test func walkMenuBarReturnsMenuBarItems() {
    // AXMenuBar has AXMenuBarItem children, each with an AXMenu child (dropdown)
    let dropdownItem1 = MockAccessibleElement(role: "AXMenuItem", label: "New")
    let dropdownItem2 = MockAccessibleElement(role: "AXMenuItem", label: "Open")
    let fileMenu = MockAccessibleElement(role: "AXMenu", label: "File", children: [dropdownItem1, dropdownItem2])
    let fileBarItem = MockAccessibleElement(
        role: "AXMenuBarItem", label: "File",
        position: CGPoint(x: 30, y: 0), size: CGSize(width: 40, height: 25),
        children: [fileMenu]
    )
    let editBarItem = MockAccessibleElement(
        role: "AXMenuBarItem", label: "Edit",
        position: CGPoint(x: 70, y: 0), size: CGSize(width: 40, height: 25),
        children: []
    )
    let menuBar = MockAccessibleElement(
        role: "AXMenuBar",
        position: CGPoint(x: 0, y: 0), size: CGSize(width: 1920, height: 25),
        children: [fileBarItem, editBarItem]
    )

    // Walk with depth=1: only get the AXMenuBarItems, not their dropdown contents
    let result = TreeWalker.walk(root: menuBar, depth: 1)

    #expect(result.count == 2)
    #expect(result[0].role == .menuItem) // AXMenuBarItem maps to menuItem
    #expect(result[0].label == "File")
    #expect(result[0].position == CGPoint(x: 30, y: 0))
    #expect(result[0].size == CGSize(width: 40, height: 25))
    #expect(result[0].children == nil) // depth=1 means no expansion
    #expect(result[0].childCount == 1) // actual child count (the AXMenu)
    #expect(result[1].role == .menuItem)
    #expect(result[1].label == "Edit")
}

@Test func walkMenuBarAtDepthThreeExpandsDropdowns() {
    // Verify that deeper depth walks into dropdown menus
    let dropdownItem = MockAccessibleElement(role: "AXMenuItem", label: "New")
    let fileMenu = MockAccessibleElement(role: "AXMenu", label: "File", children: [dropdownItem])
    let fileBarItem = MockAccessibleElement(
        role: "AXMenuBarItem", label: "File",
        children: [fileMenu]
    )
    let menuBar = MockAccessibleElement(role: "AXMenuBar", children: [fileBarItem])

    // Walk with depth=3: expands into dropdown menus
    let result = TreeWalker.walk(root: menuBar, depth: 3)

    #expect(result.count == 1)
    let fileInfo = result[0]
    #expect(fileInfo.label == "File")
    // At depth 3, children should be expanded
    #expect(fileInfo.children?.count == 1) // the AXMenu
    let menuInfo = fileInfo.children?[0]
    #expect(menuInfo?.role == .menu)
    #expect(menuInfo?.children?.count == 1)
    #expect(menuInfo?.children?[0].label == "New")
}

// MARK: - Container descent invariant

// Codifies the walker's behaviour: nested AXGroup containers (the AX shape of
// NSStackView and NSView subclasses) do NOT act as traversal barriers. Role
// filtering still reaches descendants hosted several layers deep. Regression
// guard against any future "skip container" heuristic in the walker.
@Test func walkDescendsThroughFourLevelsOfGroupContainers() {
    let deepField = MockAccessibleElement(role: "AXTextField", label: "Deep URL")
    let innermost = MockAccessibleElement(role: "AXGroup", children: [deepField])
    let midGroup = MockAccessibleElement(role: "AXGroup", children: [innermost])
    let outerGroup = MockAccessibleElement(role: "AXGroup", children: [midGroup])
    let root = MockAccessibleElement(role: "AXWindow", label: "Window", children: [outerGroup])

    let result = TreeWalker.walk(root: root, depth: 10, roleFilter: .textfield)

    // The filter keeps ancestors that have matching descendants, so we should
    // still see the outer group; but only the textfield leaf survives at the
    // bottom. Walk down the chain and assert the leaf is reachable.
    #expect(result.count == 1)
    var cursor: ElementInfo? = result[0]
    for _ in 0..<3 {
        #expect(cursor?.role == .group)
        #expect(cursor?.children?.count == 1)
        cursor = cursor?.children?[0]
    }
    #expect(cursor?.role == .textfield)
    #expect(cursor?.label == "Deep URL")
}

@Test func walkFindsTextFieldsInsideGroupSiblings() {
    // Mirrors NSStackView with two anonymous arranged subviews, each wrapping
    // a textfield. QueryResolver over the walker output should yield exactly
    // two textfield matches — confirming the walk reaches both siblings.
    let field1 = MockAccessibleElement(role: "AXTextField", label: "URL")
    let field2 = MockAccessibleElement(role: "AXTextField", label: "Search")
    let cell1 = MockAccessibleElement(role: "AXGroup", children: [field1])
    let cell2 = MockAccessibleElement(role: "AXGroup", children: [field2])
    let stack = MockAccessibleElement(role: "AXGroup", label: "Stack", children: [cell1, cell2])
    let root = MockAccessibleElement(role: "AXWindow", label: "Window", children: [stack])

    let walked = TreeWalker.walk(root: root, depth: 10, roleFilter: .textfield)
    let result = QueryResolver.resolve(in: walked, role: .textfield, label: nil, id: nil, index: nil)

    guard case .multiple(let matches) = result else {
        #expect(Bool(false), "Expected .multiple, got \(result)")
        return
    }
    #expect(matches.count == 2)
    #expect(Set(matches.compactMap { $0.label }) == ["URL", "Search"])
}
