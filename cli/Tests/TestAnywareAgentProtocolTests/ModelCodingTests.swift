import Testing
import Foundation
import TestAnywareAgentProtocol

// MARK: - ElementInfo tests

@Test func elementInfoRoundTripAllFields() throws {
    let element = ElementInfo(
        role: .button,
        label: "OK",
        value: "pressed",
        description: "Confirms the dialog",
        id: "btn-ok",
        enabled: true,
        focused: false,
        position: CGPoint(x: 10.5, y: 20.0),
        size: CGSize(width: 80.0, height: 30.0),
        childCount: 0,
        actions: ["AXPress", "AXShowMenu"],
        platformRole: "AXButton",
        children: nil
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(element)
    let decoded = try decoder.decode(ElementInfo.self, from: data)
    #expect(decoded == element)
}

@Test func elementInfoRoundTripOptionalFieldsNil() throws {
    let element = ElementInfo(
        role: .unknown,
        label: nil,
        value: nil,
        description: nil,
        id: nil,
        enabled: false,
        focused: false,
        position: nil,
        size: nil,
        childCount: 0,
        actions: [],
        platformRole: nil,
        children: nil
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(element)
    let decoded = try decoder.decode(ElementInfo.self, from: data)
    #expect(decoded == element)
}

@Test func elementInfoRoundTripWithChildren() throws {
    let child = ElementInfo(
        role: .text,
        label: "OK",
        value: nil,
        description: nil,
        id: nil,
        enabled: true,
        focused: false,
        position: CGPoint(x: 12.0, y: 22.0),
        size: CGSize(width: 20.0, height: 14.0),
        childCount: 0,
        actions: [],
        platformRole: nil,
        children: nil
    )
    let parent = ElementInfo(
        role: .button,
        label: "OK",
        value: nil,
        description: nil,
        id: "btn-ok",
        enabled: true,
        focused: true,
        position: CGPoint(x: 10.0, y: 20.0),
        size: CGSize(width: 80.0, height: 30.0),
        childCount: 1,
        actions: ["AXPress"],
        platformRole: "AXButton",
        children: [child]
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(parent)
    let decoded = try decoder.decode(ElementInfo.self, from: data)
    #expect(decoded == parent)
}

@Test func elementInfoJSONKeysCamelCase() throws {
    let element = ElementInfo(
        role: .button,
        label: "OK",
        value: nil,
        description: nil,
        id: nil,
        enabled: true,
        focused: false,
        position: CGPoint(x: 1.0, y: 2.0),
        size: CGSize(width: 10.0, height: 5.0),
        childCount: 0,
        actions: [],
        platformRole: "AXButton",
        children: nil
    )
    let data = try JSONEncoder().encode(element)
    let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    #expect(json["role"] != nil)
    #expect(json["label"] != nil)
    #expect(json["enabled"] != nil)
    #expect(json["focused"] != nil)
    #expect(json["childCount"] != nil)
    #expect(json["actions"] != nil)
    #expect(json["platformRole"] != nil)
    #expect(json["positionX"] != nil)
    #expect(json["positionY"] != nil)
    #expect(json["sizeWidth"] != nil)
    #expect(json["sizeHeight"] != nil)
    // snake_case keys must NOT appear
    #expect(json["child_count"] == nil)
    #expect(json["platform_role"] == nil)
}

// MARK: - WindowInfo tests

@Test func windowInfoRoundTripWithTitle() throws {
    let window = WindowInfo(
        title: "My App — Document",
        windowType: "window",
        size: CGSize(width: 1024.0, height: 768.0),
        position: CGPoint(x: 0.0, y: 0.0),
        appName: "MyApp",
        focused: true,
        elements: nil
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(window)
    let decoded = try decoder.decode(WindowInfo.self, from: data)
    #expect(decoded == window)
}

@Test func windowInfoRoundTripWithoutTitle() throws {
    let window = WindowInfo(
        title: nil,
        windowType: "menu",
        size: CGSize(width: 200.0, height: 300.0),
        position: CGPoint(x: 50.0, y: 100.0),
        appName: "MyApp",
        focused: false,
        elements: nil
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(window)
    let decoded = try decoder.decode(WindowInfo.self, from: data)
    #expect(decoded == window)
}

@Test func windowInfoRoundTripWithElements() throws {
    let element = ElementInfo(
        role: .button,
        label: "Close",
        value: nil,
        description: nil,
        id: nil,
        enabled: true,
        focused: false,
        position: CGPoint(x: 8.0, y: 8.0),
        size: CGSize(width: 14.0, height: 14.0),
        childCount: 0,
        actions: ["AXPress"],
        platformRole: nil,
        children: nil
    )
    let window = WindowInfo(
        title: "My Window",
        windowType: "window",
        size: CGSize(width: 800.0, height: 600.0),
        position: CGPoint(x: 100.0, y: 50.0),
        appName: "MyApp",
        focused: true,
        elements: [element]
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(window)
    let decoded = try decoder.decode(WindowInfo.self, from: data)
    #expect(decoded == window)
}

// MARK: - SnapshotResponse tests

@Test func snapshotResponseRoundTrip() throws {
    let element = ElementInfo(
        role: .textfield,
        label: "Search",
        value: "hello",
        description: nil,
        id: "search-field",
        enabled: true,
        focused: true,
        position: CGPoint(x: 20.0, y: 10.0),
        size: CGSize(width: 200.0, height: 24.0),
        childCount: 0,
        actions: ["AXConfirm"],
        platformRole: "AXTextField",
        children: nil
    )
    let window = WindowInfo(
        title: "Browser",
        windowType: "window",
        size: CGSize(width: 1200.0, height: 800.0),
        position: CGPoint(x: 0.0, y: 0.0),
        appName: "Safari",
        focused: true,
        elements: [element]
    )
    let snapshot = SnapshotResponse(windows: [window])
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(snapshot)
    let decoded = try decoder.decode(SnapshotResponse.self, from: data)
    #expect(decoded == snapshot)
}

@Test func snapshotResponseEmptyWindows() throws {
    let snapshot = SnapshotResponse(windows: [])
    let data = try JSONEncoder().encode(snapshot)
    let decoded = try JSONDecoder().decode(SnapshotResponse.self, from: data)
    #expect(decoded == snapshot)
}

// MARK: - ActionResponse tests

@Test func actionResponseSuccessWithMessage() throws {
    let response = ActionResponse(success: true, message: "Clicked successfully")
    let data = try JSONEncoder().encode(response)
    let decoded = try JSONDecoder().decode(ActionResponse.self, from: data)
    #expect(decoded == response)
}

@Test func actionResponseFailureNoMessage() throws {
    let response = ActionResponse(success: false, message: nil)
    let data = try JSONEncoder().encode(response)
    let decoded = try JSONDecoder().decode(ActionResponse.self, from: data)
    #expect(decoded == response)
}

// MARK: - ErrorResponse tests

@Test func errorResponseRoundTrip() throws {
    let response = ErrorResponse(error: "elementNotFound", details: "No element matched the given selector")
    let data = try JSONEncoder().encode(response)
    let decoded = try JSONDecoder().decode(ErrorResponse.self, from: data)
    #expect(decoded == response)
}

@Test func errorResponseNoDetails() throws {
    let response = ErrorResponse(error: "timeout", details: nil)
    let data = try JSONEncoder().encode(response)
    let decoded = try JSONDecoder().decode(ErrorResponse.self, from: data)
    #expect(decoded == response)
}

// MARK: - InspectResponse tests

@Test func inspectResponseRoundTripAllFields() throws {
    let element = ElementInfo(
        role: .text,
        label: "Hello World",
        value: nil,
        description: nil,
        id: nil,
        enabled: true,
        focused: false,
        position: CGPoint(x: 5.0, y: 5.0),
        size: CGSize(width: 100.0, height: 20.0),
        childCount: 0,
        actions: [],
        platformRole: "AXStaticText",
        children: nil
    )
    let response = InspectResponse(
        element: element,
        fontFamily: "Helvetica Neue",
        fontSize: 13.0,
        fontWeight: "regular",
        textColor: "#000000",
        bounds: CGRect(origin: CGPoint(x: 5.0, y: 5.0), size: CGSize(width: 100.0, height: 20.0))
    )
    let encoder = JSONEncoder()
    let decoder = JSONDecoder()
    let data = try encoder.encode(response)
    let decoded = try decoder.decode(InspectResponse.self, from: data)
    #expect(decoded == response)
}

@Test func inspectResponseMinimalFields() throws {
    let element = ElementInfo(
        role: .button,
        label: "Submit",
        value: nil,
        description: nil,
        id: nil,
        enabled: true,
        focused: false,
        position: nil,
        size: nil,
        childCount: 0,
        actions: [],
        platformRole: nil,
        children: nil
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
    let decoded = try JSONDecoder().decode(InspectResponse.self, from: data)
    #expect(decoded == response)
}

@Test func inspectResponseJSONKeysCamelCase() throws {
    let element = ElementInfo(
        role: .text,
        label: "Hi",
        value: nil,
        description: nil,
        id: nil,
        enabled: true,
        focused: false,
        position: nil,
        size: nil,
        childCount: 0,
        actions: [],
        platformRole: nil,
        children: nil
    )
    let response = InspectResponse(
        element: element,
        fontFamily: "Arial",
        fontSize: 12.0,
        fontWeight: "bold",
        textColor: "#FF0000",
        bounds: CGRect(origin: CGPoint(x: 0, y: 0), size: CGSize(width: 50, height: 20))
    )
    let data = try JSONEncoder().encode(response)
    let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    #expect(json["element"] != nil)
    #expect(json["fontFamily"] != nil)
    #expect(json["fontSize"] != nil)
    #expect(json["fontWeight"] != nil)
    #expect(json["textColor"] != nil)
    #expect(json["boundsX"] != nil)
    #expect(json["boundsY"] != nil)
    #expect(json["boundsWidth"] != nil)
    #expect(json["boundsHeight"] != nil)
    // snake_case must NOT appear
    #expect(json["font_family"] == nil)
    #expect(json["font_size"] == nil)
    #expect(json["font_weight"] == nil)
    #expect(json["text_color"] == nil)
}
