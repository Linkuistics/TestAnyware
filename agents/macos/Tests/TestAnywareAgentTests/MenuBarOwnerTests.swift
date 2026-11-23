import Darwin
import Testing
@testable import TestAnywareAgent

@Suite("MenuBarOwner")
struct MenuBarOwnerTests {
    private let helper: pid_t = 426
    private let dock: pid_t = 393
    private let finder: pid_t = 413
    private let safari: pid_t = 700

    private func isRegular(_ pid: pid_t) -> Bool {
        pid == finder || pid == safari
    }

    @Test("The focused app comes first")
    func focusedAppComesFirst() {
        let pids = MenuBarOwner.candidatePids(
            focused: safari, windowOrder: [finder, safari], isRegular: isRegular
        )
        #expect(pids == [safari, finder])
    }

    @Test("A focused helper with no menu bar falls through to the frontmost regular app")
    func focusedHelperFallsThrough() {
        let pids = MenuBarOwner.candidatePids(
            focused: helper, windowOrder: [dock, finder, safari], isRegular: isRegular
        )
        #expect(pids == [helper, finder, safari])
    }

    @Test("With no focused app, the frontmost regular app comes first")
    func noFocusedApp() {
        let pids = MenuBarOwner.candidatePids(
            focused: nil, windowOrder: [dock, safari, finder], isRegular: isRegular
        )
        #expect(pids == [safari, finder])
    }

    @Test("A process with several windows appears once")
    func repeatedWindowOwner() {
        let pids = MenuBarOwner.candidatePids(
            focused: finder, windowOrder: [finder, safari, finder, safari], isRegular: isRegular
        )
        #expect(pids == [finder, safari])
    }

    @Test("No focused app and no regular app gives no candidate")
    func noCandidate() {
        let pids = MenuBarOwner.candidatePids(
            focused: nil, windowOrder: [dock, helper], isRegular: isRegular
        )
        #expect(pids.isEmpty)
    }
}
