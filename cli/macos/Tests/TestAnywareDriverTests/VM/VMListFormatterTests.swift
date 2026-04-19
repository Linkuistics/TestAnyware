import Testing
import Foundation
@testable import TestAnywareDriver

@Suite("VMListFormatter")
struct VMListFormatterTests {

    @Test func emptyListRendersNonePlaceholders() {
        let output = VMListFormatter.render(goldens: [], running: [])
        let expected = """
        Golden images:
          (none)

        Running clones:
          (none)
        """
        #expect(output == expected)
    }

    @Test func goldensPreserveBashPaddingWidths() {
        let goldens = [
            VMListEntry(
                kind: .golden, name: "testanyware-golden-macos-tahoe",
                platform: "macos", backend: "tart", sizeGB: "50 GB"
            ),
            VMListEntry(
                kind: .golden, name: "testanyware-golden-linux-24.04",
                platform: "linux", backend: "tart", sizeGB: "20 GB"
            ),
            VMListEntry(
                kind: .golden, name: "testanyware-golden-windows-11",
                platform: "windows", backend: "qemu", sizeGB: "15 GB"
            ),
        ]
        let output = VMListFormatter.render(goldens: goldens, running: [])
        let expected = """
        Golden images:
          macos    testanyware-golden-macos-tahoe           tart     50 GB
          linux    testanyware-golden-linux-24.04           tart     20 GB
          windows  testanyware-golden-windows-11            qemu     15 GB

        Running clones:
          (none)
        """
        #expect(output == expected)
    }

    @Test func runningEntriesIncludeAgentVncPID() {
        let running = [
            VMListEntry(
                kind: .running, name: "testanyware-a1b2c3d4",
                platform: "macos", backend: "tart",
                agent: "agent=192.168.64.207:8648",
                vnc: "vnc=?",
                pid: 77198
            )
        ]
        let output = VMListFormatter.render(goldens: [], running: running)
        let expected = """
        Golden images:
          (none)

        Running clones:
          testanyware-a1b2c3d4 macos    agent=192.168.64.207:8648      vnc=?                    PID 77198
        """
        #expect(output == expected)
    }

    @Test func goldenWithNilSizeRendersAsQuestionMark() {
        let output = VMListFormatter.render(
            goldens: [
                VMListEntry(
                    kind: .golden, name: "testanyware-golden-windows-11",
                    platform: "windows", backend: "qemu", sizeGB: nil
                )
            ],
            running: []
        )
        #expect(output.contains("qemu     ? GB"))
    }

    @Test func runningEntryWithMissingPIDShowsQuestionMark() {
        let running = [
            VMListEntry(
                kind: .running, name: "testanyware-x1",
                platform: "windows", backend: "qemu",
                agent: "agent=localhost:?",
                vnc: "vnc=localhost:?",
                pid: nil
            )
        ]
        let output = VMListFormatter.render(goldens: [], running: running)
        #expect(output.contains("PID ?"))
    }
}
