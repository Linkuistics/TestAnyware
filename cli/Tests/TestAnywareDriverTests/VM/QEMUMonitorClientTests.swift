import Testing
@testable import TestAnywareDriver

@Suite("QEMUMonitorClient parsers")
struct QEMUMonitorClientTests {

    @Test func parsesAgentPortFromInfoUsernet() {
        let text = """
        Hub -1 (#net0):
          TCP[HOST_FORWARD]  4  *  64321  10.0.2.15  8648     0     0
        """
        #expect(QEMUMonitorClient.parseAgentPort(infoUsernet: text) == 64321)
    }

    @Test func parsesVNCPortFromInfoVNC() {
        let text = "Server:\n  Server: 127.0.0.1:5901 (ipv4)\n  Auth: vnc\n"
        #expect(QEMUMonitorClient.parseVNCPort(infoVnc: text) == 5901)
    }

    @Test func parseAgentPortReturnsNilWhenAbsent() {
        #expect(QEMUMonitorClient.parseAgentPort(infoUsernet: "nothing here") == nil)
    }

    @Test func parseVNCPortReturnsNilWhenAbsent() {
        #expect(QEMUMonitorClient.parseVNCPort(infoVnc: "no server here") == nil)
    }

    @Test func parseAgentPortReturnsFirstHostForwardRow() {
        // QEMU's hostfwd list can have several entries; the agent forward is
        // always the first one our launches register, so first-match wins.
        let text = """
        Hub -1 (#net0):
          TCP[HOST_FORWARD]  4  *  44444  10.0.2.15  22       0     0
          TCP[HOST_FORWARD]  5  *  55555  10.0.2.15  8648     0     0
        """
        #expect(QEMUMonitorClient.parseAgentPort(infoUsernet: text) == 44444)
    }

    @Test func parseVNCPortIgnoresNon127Bind() {
        // A bind to another address should not match — our QEMU invocations
        // always bind `localhost`, so foreign rows are noise.
        let text = "Server:\n  Server: 10.0.0.1:5901 (ipv4)\n"
        #expect(QEMUMonitorClient.parseVNCPort(infoVnc: text) == nil)
    }

    @Test func parseAgentPortIgnoresPromptText() {
        // HMP responses are followed by the `(qemu)` prompt — trailing noise
        // must not change the result.
        let text = """
        Hub -1 (#net0):
          TCP[HOST_FORWARD]  4  *  12345  10.0.2.15  8648     0     0
        (qemu)\u{0020}
        """
        #expect(QEMUMonitorClient.parseAgentPort(infoUsernet: text) == 12345)
    }
}
