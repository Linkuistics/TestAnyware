import Testing
import Foundation
@testable import TestAnywareDriver

/// Guards the `?path=` percent-encoding contract for streaming file transfer
/// (ADR-0001). The Swift client must produce byte-identical query strings to
/// the Rust client's `NON_ALPHANUMERIC` encoding, because that is the one
/// scheme that decodes the same across all three agents — notably Python's
/// `parse_qs`, which reads a literal `+` as a space. The two traps this test
/// pins: `+` must encode to `%2B` (not `+`), and non-ASCII path characters
/// must encode every UTF-8 byte (so `URLComponents`/`.alphanumerics` Unicode
/// leakage is ruled out).
@Suite("AgentFileTransferURL")
struct AgentFileTransferURLTests {

    private let client = AgentTCPClient(host: "10.0.0.5", port: 8648)

    @Test func encodesSpaceAndPlusPerRFC3986() {
        let url = client.fileTransferURL("/upload", path: "/tmp/my docs/a+b.bin")
        #expect(
            url.absoluteString
                == "http://10.0.0.5:8648/upload?path=%2Ftmp%2Fmy%20docs%2Fa%2Bb%2Ebin"
        )
    }

    @Test func encodesEveryByteOfNonASCIIPath() {
        // café → the UTF-8 bytes of é (0xC3 0xA9) must both be percent-encoded.
        let url = client.fileTransferURL("/download", path: "/tmp/café.txt")
        #expect(
            url.absoluteString
                == "http://10.0.0.5:8648/download?path=%2Ftmp%2Fcaf%C3%A9%2Etxt"
        )
    }
}
