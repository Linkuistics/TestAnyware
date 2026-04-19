import ArgumentParser
import Foundation
import TestAnywareDriver
import TestAnywareAgentProtocol

// MARK: - Window Resolution Helper

/// Resolves the WindowInfo for the given filter string via the agent TCP service.
/// Throws ValidationError if the agent is not configured or the window is not found.
func resolveWindow(connection: ConnectionOptions, windowFilter: String) async throws -> WindowInfo {
    let agent = try connection.resolveAgent()
    let response = try await agent.windows()
    guard let window = response.windows.first(where: { win in
        (win.title?.localizedCaseInsensitiveContains(windowFilter) ?? false) ||
        win.appName.localizedCaseInsensitiveContains(windowFilter)
    }) else {
        throw ValidationError("No window matching '\(windowFilter)'")
    }
    return window
}

// MARK: - Input Command

struct InputCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "input",
        abstract: "Send keyboard and mouse input",
        subcommands: [
            KeyPressCommand.self,
            KeyDownCommand.self,
            KeyUpCommand.self,
            TypeCommand.self,
            ClickCommand.self,
            MouseDownCommand.self,
            MouseUpCommand.self,
            MoveCommand.self,
            ScrollCommand.self,
            DragCommand.self,
        ]
    )
}

struct KeyPressCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "key", abstract: "Press a key")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Key name (e.g. return, tab, a, f1)")
    var key: String

    @Option(name: .shortAndLong, help: "Modifier keys (comma-separated: cmd,shift,alt,ctrl)")
    var modifiers: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        let mods = modifiers?.split(separator: ",").map(String.init) ?? []
        try await client.pressKey(key, modifiers: mods)
        print("Key pressed: \(key)\(mods.isEmpty ? "" : " + \(mods.joined(separator: "+"))")")
    }
}

struct KeyDownCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "key-down", abstract: "Send key-down (without releasing)")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Key name (e.g. shift, cmd, a)")
    var key: String

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        try await client.keyDown(key)
        print("Key down: \(key)")
    }
}

struct KeyUpCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "key-up", abstract: "Send key-up (release)")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Key name (e.g. shift, cmd, a)")
    var key: String

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        try await client.keyUp(key)
        print("Key up: \(key)")
    }
}

struct TypeCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "type", abstract: "Type text")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Text to type")
    var text: String

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        try await client.typeText(text)
        print("Typed: \(text)")
    }
}

struct ClickCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "click", abstract: "Click at coordinates")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "X coordinate")
    var x: Int

    @Argument(help: "Y coordinate")
    var y: Int

    @Option(name: .shortAndLong, help: "Mouse button (left, right, middle)")
    var button: String = "left"

    @Option(name: .shortAndLong, help: "Click count")
    var count: Int = 1

    @Option(
        name: .long,
        help: "Window name for relative coordinates. Caveat on macOS Tahoe: AX-reported window origin includes the drop-shadow inset (~40px), so clicks land below intent. Prefer screen-absolute coords from `testanyware screenshot` when precision matters."
    )
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        var offsetX = 0, offsetY = 0
        if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            offsetX = Int(win.position.x)
            offsetY = Int(win.position.y)
        }

        try await client.click(x: x + offsetX, y: y + offsetY, button: button, count: count)
        print("Clicked at (\(x + offsetX), \(y + offsetY)) button=\(button) count=\(count)")
    }
}

struct MouseDownCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "mouse-down", abstract: "Press mouse button (without releasing)")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "X coordinate")
    var x: Int

    @Argument(help: "Y coordinate")
    var y: Int

    @Option(name: .shortAndLong, help: "Mouse button (left, right, middle)")
    var button: String = "left"

    @Option(
        name: .long,
        help: "Window name for relative coordinates. Caveat on macOS Tahoe: AX-reported window origin includes the drop-shadow inset (~40px), so clicks land below intent. Prefer screen-absolute coords from `testanyware screenshot` when precision matters."
    )
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        var offsetX = 0, offsetY = 0
        if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            offsetX = Int(win.position.x)
            offsetY = Int(win.position.y)
        }

        try await client.mouseDown(x: x + offsetX, y: y + offsetY, button: button)
        print("Mouse down at (\(x + offsetX), \(y + offsetY)) button=\(button)")
    }
}

struct MouseUpCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "mouse-up", abstract: "Release mouse button")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "X coordinate")
    var x: Int

    @Argument(help: "Y coordinate")
    var y: Int

    @Option(name: .shortAndLong, help: "Mouse button (left, right, middle)")
    var button: String = "left"

    @Option(
        name: .long,
        help: "Window name for relative coordinates. Caveat on macOS Tahoe: AX-reported window origin includes the drop-shadow inset (~40px), so clicks land below intent. Prefer screen-absolute coords from `testanyware screenshot` when precision matters."
    )
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        var offsetX = 0, offsetY = 0
        if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            offsetX = Int(win.position.x)
            offsetY = Int(win.position.y)
        }

        try await client.mouseUp(x: x + offsetX, y: y + offsetY, button: button)
        print("Mouse up at (\(x + offsetX), \(y + offsetY)) button=\(button)")
    }
}

struct MoveCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "move", abstract: "Move mouse")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "X coordinate")
    var x: Int

    @Argument(help: "Y coordinate")
    var y: Int

    @Option(
        name: .long,
        help: "Window name for relative coordinates. Caveat on macOS Tahoe: AX-reported window origin includes the drop-shadow inset (~40px), so coords land below intent. Prefer screen-absolute coords from `testanyware screenshot` when precision matters."
    )
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        var offsetX = 0, offsetY = 0
        if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            offsetX = Int(win.position.x)
            offsetY = Int(win.position.y)
        }

        try await client.mouseMove(x: x + offsetX, y: y + offsetY)
        print("Mouse moved to (\(x + offsetX), \(y + offsetY))")
    }
}

struct ScrollCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "scroll", abstract: "Scroll at coordinates")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "X coordinate")
    var x: Int

    @Argument(help: "Y coordinate")
    var y: Int

    @Option(name: .long, help: "Horizontal scroll amount (negative=left)")
    var dx: Int = 0

    @Option(name: .long, help: "Vertical scroll amount (negative=up)")
    var dy: Int = 0

    @Option(
        name: .long,
        help: "Window name for relative coordinates. Caveat on macOS Tahoe: AX-reported window origin includes the drop-shadow inset (~40px), so coords land below intent. Prefer screen-absolute coords from `testanyware screenshot` when precision matters."
    )
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        var offsetX = 0, offsetY = 0
        if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            offsetX = Int(win.position.x)
            offsetY = Int(win.position.y)
        }

        try await client.scroll(x: x + offsetX, y: y + offsetY, dx: dx, dy: dy)
        print("Scrolled at (\(x + offsetX), \(y + offsetY)) dx=\(dx) dy=\(dy)")
    }
}

struct DragCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "drag", abstract: "Drag from one point to another")

    @OptionGroup var connection: ConnectionOptions

    @Argument(help: "Start X")
    var fromX: Int

    @Argument(help: "Start Y")
    var fromY: Int

    @Argument(help: "End X")
    var toX: Int

    @Argument(help: "End Y")
    var toY: Int

    @Option(name: .shortAndLong, help: "Mouse button")
    var button: String = "left"

    @Option(name: .shortAndLong, help: "Number of interpolation steps")
    var steps: Int = 10

    @Option(
        name: .long,
        help: "Window name for relative coordinates. Caveat on macOS Tahoe: AX-reported window origin includes the drop-shadow inset (~40px), so coords land below intent. Prefer screen-absolute coords from `testanyware screenshot` when precision matters."
    )
    var window: String?

    mutating func run() async throws {
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        var offsetX = 0, offsetY = 0
        if let windowFilter = window {
            let win = try await resolveWindow(connection: connection, windowFilter: windowFilter)
            offsetX = Int(win.position.x)
            offsetY = Int(win.position.y)
        }

        try await client.drag(
            fromX: fromX + offsetX, fromY: fromY + offsetY,
            toX: toX + offsetX, toY: toY + offsetY,
            button: button, steps: steps
        )
        print("Dragged from (\(fromX + offsetX),\(fromY + offsetY)) to (\(toX + offsetX),\(toY + offsetY))")
    }
}
