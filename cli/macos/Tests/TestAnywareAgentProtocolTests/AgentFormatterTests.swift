import Testing
import Foundation
import TestAnywareAgentProtocol

// MARK: - Snapshot formatting

@Test func formatSnapshotNestedElements() throws {
    let snapshot = SnapshotResponse(windows: [
        WindowInfo(
            title: "My App",
            windowType: "window",
            size: CGSize(width: 800, height: 600),
            position: CGPoint(x: 0, y: 0),
            appName: "Xcode",
            focused: false,
            elements: [
                ElementInfo(
                    role: .toolbar, label: nil, value: nil, description: nil, id: nil,
                    enabled: true, focused: false, position: nil, size: nil,
                    childCount: 2, actions: [], platformRole: nil,
                    children: [
                        ElementInfo(
                            role: .button, label: "New", value: nil, description: nil, id: nil,
                            enabled: true, focused: false, position: nil, size: nil,
                            childCount: 0, actions: ["AXPress"], platformRole: nil, children: nil
                        ),
                        ElementInfo(
                            role: .button, label: "Save", value: nil, description: nil, id: nil,
                            enabled: true, focused: false, position: nil, size: nil,
                            childCount: 0, actions: ["AXPress"], platformRole: nil, children: nil
                        ),
                    ]
                ),
            ]
        ),
    ])
    let data = try JSONEncoder().encode(snapshot)
    let output = try AgentFormatter.formatSnapshot(data)
    let expected = """
        "My App" (window) 800x600 app:"Xcode"
          toolbar
            button "New"
            button "Save"
        """
    #expect(output == expected)
}

@Test func formatSnapshotDisabledAndFocusedElements() throws {
    let snapshot = SnapshotResponse(windows: [
        WindowInfo(
            title: "Editor",
            windowType: "window",
            size: CGSize(width: 1024, height: 768),
            position: CGPoint(x: 0, y: 0),
            appName: "TextEdit",
            focused: true,
            elements: [
                ElementInfo(
                    role: .button, label: "Undo", value: nil, description: nil, id: nil,
                    enabled: false, focused: false, position: nil, size: nil,
                    childCount: 0, actions: [], platformRole: nil, children: nil
                ),
                ElementInfo(
                    role: .textfield, label: "Search...", value: "", description: nil, id: nil,
                    enabled: true, focused: true, position: nil, size: nil,
                    childCount: 0, actions: [], platformRole: nil, children: nil
                ),
            ]
        ),
    ])
    let data = try JSONEncoder().encode(snapshot)
    let output = try AgentFormatter.formatSnapshot(data)
    let expected = """
        "Editor" (window) 1024x768 [focused] app:"TextEdit"
          button "Undo" [disabled]
          textfield "Search..." [focused] value=""
        """
    #expect(output == expected)
}

@Test func formatSnapshotElementWithValue() throws {
    let snapshot = SnapshotResponse(windows: [
        WindowInfo(
            title: "Form",
            windowType: "window",
            size: CGSize(width: 400, height: 300),
            position: CGPoint(x: 0, y: 0),
            appName: "Safari",
            focused: false,
            elements: [
                ElementInfo(
                    role: .textfield, label: "Name", value: "Alice", description: nil, id: nil,
                    enabled: true, focused: false, position: nil, size: nil,
                    childCount: 0, actions: [], platformRole: nil, children: nil
                ),
            ]
        ),
    ])
    let data = try JSONEncoder().encode(snapshot)
    let output = try AgentFormatter.formatSnapshot(data)
    let expected = """
        "Form" (window) 400x300 app:"Safari"
          textfield "Name" value="Alice"
        """
    #expect(output == expected)
}

@Test func formatSnapshotWindowWithoutTitle() throws {
    let snapshot = SnapshotResponse(windows: [
        WindowInfo(
            title: nil,
            windowType: "menu",
            size: CGSize(width: 200, height: 300),
            position: CGPoint(x: 50, y: 100),
            appName: "Finder",
            focused: false,
            elements: nil
        ),
    ])
    let data = try JSONEncoder().encode(snapshot)
    let output = try AgentFormatter.formatSnapshot(data)
    let expected = """
        (menu) 200x300 app:"Finder"
        """
    #expect(output == expected)
}

// MARK: - Windows formatting

@Test func formatWindowsListsWindowsWithAnnotations() throws {
    let snapshot = SnapshotResponse(windows: [
        WindowInfo(
            title: "My App - main.swift",
            windowType: "window",
            size: CGSize(width: 800, height: 600),
            position: CGPoint(x: 0, y: 0),
            appName: "Xcode",
            focused: true,
            elements: nil
        ),
        WindowInfo(
            title: "Console",
            windowType: "window",
            size: CGSize(width: 600, height: 400),
            position: CGPoint(x: 100, y: 100),
            appName: "Terminal",
            focused: false,
            elements: nil
        ),
    ])
    let data = try JSONEncoder().encode(snapshot)
    let output = try AgentFormatter.formatWindows(data)
    let expected = """
        "My App - main.swift" (window) 800x600 [focused] app:"Xcode"
        "Console" (window) 600x400 app:"Terminal"
        """
    #expect(output == expected)
}

// MARK: - Action formatting

@Test func formatActionSuccess() throws {
    let response = ActionResponse(success: true, message: nil)
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatAction(data)
    #expect(output == "OK")
}

@Test func formatActionSuccessWithMessage() throws {
    let response = ActionResponse(success: true, message: "Pressed button \"Save\"")
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatAction(data)
    #expect(output == "OK: Pressed button \"Save\"")
}

@Test func formatActionFailure() throws {
    let response = ActionResponse(success: false, message: nil)
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatAction(data)
    #expect(output == "FAILED")
}

@Test func formatActionFailureWithMessage() throws {
    let response = ActionResponse(success: false, message: "element not found")
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatAction(data)
    #expect(output == "FAILED: element not found")
}

// MARK: - Error formatting

@Test func formatErrorWithDetails() throws {
    let response = ErrorResponse(error: "elementNotFound", details: "No element matched the given selector")
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatError(data)
    #expect(output == "Error: elementNotFound \u{2014} No element matched the given selector")
}

@Test func formatErrorWithoutDetails() throws {
    let response = ErrorResponse(error: "timeout", details: nil)
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatError(data)
    #expect(output == "Error: timeout")
}

// MARK: - Inspect formatting

@Test func formatInspectWithAllFields() throws {
    let element = ElementInfo(
        role: .button, label: "Save", value: nil, description: nil, id: nil,
        enabled: true, focused: false, position: nil, size: nil,
        childCount: 0, actions: ["AXPress"], platformRole: nil, children: nil
    )
    let response = InspectResponse(
        element: element,
        fontFamily: "SF Pro Display",
        fontSize: 14.0,
        fontWeight: "bold",
        textColor: nil,
        bounds: CGRect(origin: CGPoint(x: 100, y: 200), size: CGSize(width: 400, height: 50))
    )
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatInspect(data)
    let expected = """
        button "Save"
          bounds: 100,200 400x50
          font: SF Pro Display 14pt bold
        """
    #expect(output == expected)
}

@Test func formatInspectMinimalFields() throws {
    let element = ElementInfo(
        role: .group, label: nil, value: nil, description: nil, id: nil,
        enabled: true, focused: false, position: nil, size: nil,
        childCount: 3, actions: [], platformRole: nil, children: nil
    )
    let response = InspectResponse(
        element: element,
        fontFamily: nil,
        fontSize: nil,
        fontWeight: nil,
        textColor: nil,
        bounds: nil
    )
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatInspect(data)
    #expect(output == "group")
}

@Test func formatInspectWithDisabledAndFocused() throws {
    let element = ElementInfo(
        role: .textfield, label: "Email", value: "test@example.com", description: nil, id: nil,
        enabled: false, focused: true, position: nil, size: nil,
        childCount: 0, actions: [], platformRole: nil, children: nil
    )
    let response = InspectResponse(
        element: element,
        fontFamily: nil,
        fontSize: nil,
        fontWeight: nil,
        textColor: nil,
        bounds: CGRect(origin: CGPoint(x: 50, y: 100), size: CGSize(width: 200, height: 30))
    )
    let data = try JSONEncoder().encode(response)
    let output = try AgentFormatter.formatInspect(data)
    let expected = """
        textfield "Email" [disabled] [focused] value="test@example.com"
          bounds: 50,100 200x30
        """
    #expect(output == expected)
}

// MARK: - List child count

@Test func formatSnapshotListWithChildCount() throws {
    let snapshot = SnapshotResponse(windows: [
        WindowInfo(
            title: "Items",
            windowType: "window",
            size: CGSize(width: 400, height: 300),
            position: CGPoint(x: 0, y: 0),
            appName: "Finder",
            focused: false,
            elements: [
                ElementInfo(
                    role: .list, label: "Files", value: nil, description: nil, id: nil,
                    enabled: true, focused: false, position: nil, size: nil,
                    childCount: 3, actions: [], platformRole: nil,
                    children: [
                        ElementInfo(
                            role: .listItem, label: "First", value: nil, description: nil, id: nil,
                            enabled: true, focused: false, position: nil, size: nil,
                            childCount: 0, actions: [], platformRole: nil, children: nil
                        ),
                        ElementInfo(
                            role: .listItem, label: "Second", value: nil, description: nil, id: nil,
                            enabled: true, focused: false, position: nil, size: nil,
                            childCount: 0, actions: [], platformRole: nil, children: nil
                        ),
                        ElementInfo(
                            role: .listItem, label: "Third", value: nil, description: nil, id: nil,
                            enabled: true, focused: false, position: nil, size: nil,
                            childCount: 0, actions: [], platformRole: nil, children: nil
                        ),
                    ]
                ),
            ]
        ),
    ])
    let data = try JSONEncoder().encode(snapshot)
    let output = try AgentFormatter.formatSnapshot(data)
    let expected = """
        "Items" (window) 400x300 app:"Finder"
          list "Files" 3 items
            list-item "First"
            list-item "Second"
            list-item "Third"
        """
    #expect(output == expected)
}
