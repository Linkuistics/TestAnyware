import Testing
import Foundation
import TestAnywareAgentProtocol

// Cross-language contract tests.
//
// Each fixture under `cli-rs/tests/fixtures/protocol/` is a canonical
// JSON document that BOTH Swift and Rust must accept and re-emit
// without losing or renaming fields. This file is the Swift half of the
// contract; `cli-rs/crates/testanyware-protocol/tests/fixtures.rs` is
// the Rust half. Either side breaking the fixture format fails one of
// these suites loudly.
//
// "Semantic equality" here means: decode the fixture into the typed
// Swift value, re-encode that value, decode the re-encoded JSON again,
// and verify the second decode equals the first. Byte-equality cannot
// be required because Swift's `JSONEncoder` does not guarantee key
// order across runs.
//
// Additionally, the test asserts that re-encoding produces the *same
// set of top-level keys* as the original fixture (i.e. no fields
// silently dropped, no fields renamed in Swift but not in the
// fixture). This is the drift-catcher.

private enum FixtureLoader {

    static func url(_ name: String) -> URL {
        // The test target lives at
        //   cli/Tests/TestAnywareAgentProtocolTests/
        // Fixtures live at the sibling Rust workspace:
        //   cli-rs/tests/fixtures/protocol/
        // Resolve via this file's source location so paths are stable
        // regardless of the working directory `swift test` is invoked
        // from.
        let thisFile = URL(fileURLWithPath: #filePath)
        return thisFile
            .deletingLastPathComponent()                      // TestAnywareAgentProtocolTests
            .deletingLastPathComponent()                      // Tests
            .deletingLastPathComponent()                      // cli
            .deletingLastPathComponent()                      // repo root
            .appendingPathComponent("cli-rs")
            .appendingPathComponent("tests")
            .appendingPathComponent("fixtures")
            .appendingPathComponent("protocol")
            .appendingPathComponent(name)
    }

    static func data(_ name: String) throws -> Data {
        try Data(contentsOf: url(name))
    }
}

private func keys(of data: Data) throws -> Set<String> {
    let object = try JSONSerialization.jsonObject(with: data, options: [])
    guard let dict = object as? [String: Any] else {
        return []
    }
    return Set(dict.keys)
}

private func roundTrip<T: Codable & Equatable>(
    _ name: String,
    _ type: T.Type
) throws {
    let raw = try FixtureLoader.data(name)
    let decoded = try JSONDecoder().decode(T.self, from: raw)
    let reEncoded = try JSONEncoder().encode(decoded)
    let decodedAgain = try JSONDecoder().decode(T.self, from: reEncoded)
    #expect(decoded == decodedAgain, "round-trip changed value for \(name)")

    let originalKeys = try keys(of: raw)
    let reEncodedKeys = try keys(of: reEncoded)
    #expect(
        originalKeys == reEncodedKeys,
        "key set drift for \(name): original=\(originalKeys.sorted()) reEncoded=\(reEncodedKeys.sorted())"
    )
}

// MARK: - ElementInfo

@Test func crossLangElementInfoFull() throws {
    try roundTrip("element-info-full.json", ElementInfo.self)
}

@Test func crossLangElementInfoMinimal() throws {
    try roundTrip("element-info-minimal.json", ElementInfo.self)
}

@Test func crossLangElementInfoWithChildren() throws {
    try roundTrip("element-info-with-children.json", ElementInfo.self)
}

// MARK: - WindowInfo

@Test func crossLangWindowInfoWithTitle() throws {
    try roundTrip("window-info-with-title.json", WindowInfo.self)
}

@Test func crossLangWindowInfoWithoutTitle() throws {
    try roundTrip("window-info-without-title.json", WindowInfo.self)
}

@Test func crossLangWindowInfoWithElements() throws {
    try roundTrip("window-info-with-elements.json", WindowInfo.self)
}

// MARK: - SnapshotResponse

@Test func crossLangSnapshotResponseTypical() throws {
    try roundTrip("snapshot-response-typical.json", SnapshotResponse.self)
}

@Test func crossLangSnapshotResponseEmpty() throws {
    try roundTrip("snapshot-response-empty.json", SnapshotResponse.self)
}

// MARK: - ActionResponse

@Test func crossLangActionResponseSuccess() throws {
    try roundTrip("action-response-success.json", ActionResponse.self)
}

@Test func crossLangActionResponseFailure() throws {
    try roundTrip("action-response-failure.json", ActionResponse.self)
}

// MARK: - ErrorResponse

@Test func crossLangErrorResponseWithDetails() throws {
    try roundTrip("error-response-with-details.json", ErrorResponse.self)
}

@Test func crossLangErrorResponseNoDetails() throws {
    try roundTrip("error-response-no-details.json", ErrorResponse.self)
}

// MARK: - InspectResponse

@Test func crossLangInspectResponseFull() throws {
    try roundTrip("inspect-response-full.json", InspectResponse.self)
}

@Test func crossLangInspectResponseMinimal() throws {
    try roundTrip("inspect-response-minimal.json", InspectResponse.self)
}
