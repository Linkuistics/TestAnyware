import Testing
@testable import TestAnywareDriver

@Suite("Smoke")
struct SmokeTests {
    @Test func moduleImports() {
        // Verifies the module compiles and can be imported.
        #expect(true)
    }
}
