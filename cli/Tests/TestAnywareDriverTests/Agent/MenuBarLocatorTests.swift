import Testing
import Foundation
@testable import TestAnywareDriver
import TestAnywareAgentProtocol

@Suite("MenuBarLocator")
struct MenuBarLocatorTests {

    // MARK: - Helpers

    private func makeElement(
        label: String?,
        position: CGPoint? = nil,
        size: CGSize? = nil,
        children: [ElementInfo]? = nil
    ) -> ElementInfo {
        ElementInfo(
            role: .menuItem,
            label: label,
            value: nil,
            description: nil,
            id: nil,
            enabled: true,
            focused: false,
            showing: nil,
            position: position,
            size: size,
            childCount: children?.count ?? 0,
            actions: [],
            platformRole: nil,
            children: children
        )
    }

    private func makeWindow(elements: [ElementInfo]?) -> WindowInfo {
        WindowInfo(
            title: "Menu Bar",
            windowType: "menuBar",
            size: CGSize(width: 1920, height: 24),
            position: CGPoint(x: 0, y: 0),
            appName: "SystemUIServer",
            focused: false,
            elements: elements
        )
    }

    // MARK: - findElement

    @Test func findElementMatchesAtTopLevel() {
        let target = makeElement(label: "File")
        let win = makeWindow(elements: [makeElement(label: "Apple"), target])
        let hit = MenuBarLocator.findElement(byLabel: "File", in: [win])
        #expect(hit?.label == "File")
    }

    @Test func findElementSearchesNestedChildren() {
        let nested = makeElement(label: "Save")
        let parent = makeElement(label: "File", children: [nested])
        let win = makeWindow(elements: [parent])
        let hit = MenuBarLocator.findElement(byLabel: "Save", in: [win])
        #expect(hit?.label == "Save")
    }

    @Test func findElementIsCaseInsensitive() {
        let target = makeElement(label: "FILE")
        let win = makeWindow(elements: [target])
        let hit = MenuBarLocator.findElement(byLabel: "file", in: [win])
        #expect(hit?.label == "FILE")
    }

    @Test func findElementReturnsNilWhenNoMatch() {
        let win = makeWindow(elements: [makeElement(label: "Edit")])
        #expect(MenuBarLocator.findElement(byLabel: "View", in: [win]) == nil)
    }

    @Test func findElementHandlesEmptyOrNilElementLists() {
        let nilWin = makeWindow(elements: nil)
        let emptyWin = makeWindow(elements: [])
        #expect(MenuBarLocator.findElement(byLabel: "File", in: [nilWin, emptyWin]) == nil)
    }

    @Test func findElementSearchesAcrossWindows() {
        let win1 = makeWindow(elements: [makeElement(label: "Apple")])
        let win2 = makeWindow(elements: [makeElement(label: "File")])
        let hit = MenuBarLocator.findElement(byLabel: "File", in: [win1, win2])
        #expect(hit?.label == "File")
    }

    // MARK: - centerPoint

    @Test func centerPointReturnsRoundedCenter() {
        let elem = makeElement(
            label: "File",
            position: CGPoint(x: 100, y: 4),
            size: CGSize(width: 40, height: 24)
        )
        let center = MenuBarLocator.centerPoint(of: elem)
        #expect(center?.x == 120)
        #expect(center?.y == 16)
    }

    @Test func centerPointRoundsHalfWidthAndHeight() {
        let elem = makeElement(
            label: "View",
            position: CGPoint(x: 200, y: 0),
            size: CGSize(width: 41, height: 23)
        )
        // (200 + 20.5) → 221 (banker's: nearest), (0 + 11.5) → 12
        let center = MenuBarLocator.centerPoint(of: elem)
        #expect(center?.x == 221)
        #expect(center?.y == 12)
    }

    @Test func centerPointReturnsNilWithoutPosition() {
        let elem = makeElement(label: "File", position: nil, size: CGSize(width: 40, height: 24))
        #expect(MenuBarLocator.centerPoint(of: elem) == nil)
    }

    @Test func centerPointReturnsNilWithoutSize() {
        let elem = makeElement(label: "File", position: CGPoint(x: 100, y: 4), size: nil)
        #expect(MenuBarLocator.centerPoint(of: elem) == nil)
    }
}
