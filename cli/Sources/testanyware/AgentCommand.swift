import ArgumentParser
import Foundation
import TestAnywareAgentProtocol
import TestAnywareDriver

struct AgentCommand: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "agent",
        abstract: "Interact with the in-VM accessibility agent",
        subcommands: [
            AgentHealthCmd.self,
            AgentSnapshotCmd.self,
            AgentInspectCmd.self,
            AgentPressCmd.self,
            AgentSetValueCmd.self,
            AgentFocusCmd.self,
            AgentShowMenuCmd.self,
            AgentWindowsCmd.self,
            AgentWindowFocusCmd.self,
            AgentWindowResizeCmd.self,
            AgentWindowMoveCmd.self,
            AgentWindowCloseCmd.self,
            AgentWindowMinimizeCmd.self,
            AgentWaitCmd.self,
        ]
    )
}

// MARK: - Shared Option Groups

struct AgentQueryOptions: ParsableArguments {
    @Option(name: .long, help: "Element role filter")
    var role: String?

    @Option(name: .long, help: "Element label filter")
    var label: String?

    @Option(name: .long, help: "Element ID filter")
    var id: String?

    @Option(name: .long, help: "Element index (0-based)")
    var index: Int?
}

struct AgentWindowFilter: ParsableArguments {
    @Option(name: .long, help: "Window title or app name filter")
    var window: String?
}

// MARK: - Health

struct AgentHealthCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "health",
        abstract: "Check agent health"
    )
    @OptionGroup var connection: ConnectionOptions

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let ok = try await agent.health()
        print(ok ? "OK" : "UNHEALTHY")
        if !ok { throw ExitCode.failure }
    }
}

// MARK: - Snapshot

struct AgentSnapshotCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "snapshot",
        abstract: "Capture accessibility element tree snapshot"
    )

    @OptionGroup var connection: ConnectionOptions

    @Option(name: .long, help: "Snapshot mode (full, interactive)")
    var mode: String?

    @Option(name: .long, help: "Window title or app name filter")
    var window: String?

    @Option(name: .long, help: "Element role filter")
    var role: String?

    @Option(name: .long, help: "Element label filter")
    var label: String?

    @Option(name: .long, help: "Maximum tree depth")
    var depth: Int?

    @Option(
        name: .long,
        help: """
        Menu-bar item label (or comma-separated path) to open via VNC click \
        before snapshotting. macOS menu submenus are lazy in the AX tree — \
        they only appear once the parent menu is open. Pass a path like \
        "File,Open Recent" to drill into a submenu; each segment is clicked \
        in order with a 400 ms settle. The deepest opened menu is left \
        visible; press Escape with `input key escape` to close it afterwards.
        """
    )
    var openMenu: String?

    @Flag(name: .long, help: "Output raw JSON instead of formatted text")
    var json: Bool = false

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        if let openMenu {
            try await openMenuBarPath(rawPath: openMenu, agent: agent)
        }
        let response = try await agent.snapshot(mode: mode, window: window, role: role, label: label, depth: depth)
        if json {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(response)
            print(String(data: data, encoding: .utf8)!)
        } else {
            print(AgentFormatter.formatSnapshot(response))
        }
    }

    /// Walk a comma-separated `--open-menu` path, clicking each segment in
    /// turn. Re-snapshots between segments with a depth that exposes the
    /// just-opened submenu's items so `MenuBarLocator.findElement` can locate
    /// the next segment within the same Menu Bar pseudo-window subtree.
    private func openMenuBarPath(rawPath: String, agent: AgentTCPClient) async throws {
        guard let segments = MenuBarLocator.parsePath(rawPath) else {
            throw ValidationError(
                "--open-menu path must be non-empty and contain no blank segments: '\(rawPath)'"
            )
        }
        let spec = try connection.resolve()
        let client = try await ServerClient.ensure(spec: spec)

        for (index, segment) in segments.enumerated() {
            // depth grows with each segment: AXMenuBar → AXMenuBarItem (1) →
            // AXMenu (2) → AXMenuItem (3) → AXMenu (4) → ... — covers up to a
            // 4-level deep nested submenu path.
            let snapshotDepth = max(3, 2 * (index + 1) + 1)
            let menuBar = try await agent.snapshot(
                mode: nil, window: "Menu Bar", role: nil, label: nil, depth: snapshotDepth
            )
            guard let element = MenuBarLocator.findElement(byLabel: segment, in: menuBar.windows) else {
                throw ValidationError(
                    "No menu item matching '\(segment)' in --open-menu path '\(rawPath)'"
                )
            }
            guard let target = MenuBarLocator.centerPoint(of: element) else {
                throw ValidationError(
                    "Menu item '\(segment)' has no position/size; cannot derive click target"
                )
            }
            try await client.click(x: target.x, y: target.y, button: "left", count: 1)
            // Brief settle for the menu animation; tested empirically as enough
            // for AppKit menus on Tahoe to populate the AX tree.
            try await Task.sleep(nanoseconds: 400_000_000)
        }
    }
}

// MARK: - Inspect

struct AgentInspectCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "inspect",
        abstract: "Inspect a single element in detail"
    )

    @OptionGroup var connection: ConnectionOptions
    @OptionGroup var query: AgentQueryOptions
    @OptionGroup var windowFilter: AgentWindowFilter

    @Flag(name: .long, help: "Output raw JSON instead of formatted text")
    var json: Bool = false

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.inspect(
            role: query.role, label: query.label,
            window: windowFilter.window, id: query.id, index: query.index
        )
        if json {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(response)
            print(String(data: data, encoding: .utf8)!)
        } else {
            print(AgentFormatter.formatInspect(response))
        }
    }
}

// MARK: - Press

struct AgentPressCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "press",
        abstract: "Press (activate) an element"
    )

    @OptionGroup var connection: ConnectionOptions
    @OptionGroup var query: AgentQueryOptions
    @OptionGroup var windowFilter: AgentWindowFilter

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.press(
            role: query.role, label: query.label,
            window: windowFilter.window, id: query.id, index: query.index
        )
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Set Value

struct AgentSetValueCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "set-value",
        abstract: "Set the value of an element"
    )

    @OptionGroup var connection: ConnectionOptions
    @OptionGroup var query: AgentQueryOptions
    @OptionGroup var windowFilter: AgentWindowFilter

    @Option(name: .long, help: "Value to set")
    var value: String

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.setValue(
            role: query.role, label: query.label,
            window: windowFilter.window, id: query.id, index: query.index,
            value: value
        )
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Focus

struct AgentFocusCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "focus",
        abstract: "Focus an element"
    )

    @OptionGroup var connection: ConnectionOptions
    @OptionGroup var query: AgentQueryOptions
    @OptionGroup var windowFilter: AgentWindowFilter

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.focus(
            role: query.role, label: query.label,
            window: windowFilter.window, id: query.id, index: query.index
        )
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Show Menu

struct AgentShowMenuCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "show-menu",
        abstract: "Show the context menu of an element"
    )

    @OptionGroup var connection: ConnectionOptions
    @OptionGroup var query: AgentQueryOptions
    @OptionGroup var windowFilter: AgentWindowFilter

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.showMenu(
            role: query.role, label: query.label,
            window: windowFilter.window, id: query.id, index: query.index
        )
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Windows

struct AgentWindowsCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "windows",
        abstract: "List all windows"
    )

    @OptionGroup var connection: ConnectionOptions

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.windows()
        print(AgentFormatter.formatWindows(response))
    }
}

// MARK: - Window Focus

struct AgentWindowFocusCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "window-focus", abstract: "Focus a window")
    @OptionGroup var connection: ConnectionOptions
    @Option(name: .long, help: "Window title or app name filter") var window: String

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.windowFocus(window: window)
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Window Resize

struct AgentWindowResizeCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "window-resize", abstract: "Resize a window")
    @OptionGroup var connection: ConnectionOptions
    @Option(name: .long, help: "Window title or app name filter") var window: String
    @Option(name: .long, help: "New width") var width: Int
    @Option(name: .long, help: "New height") var height: Int

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.windowResize(window: window, width: width, height: height)
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Window Move

struct AgentWindowMoveCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "window-move", abstract: "Move a window")
    @OptionGroup var connection: ConnectionOptions
    @Option(name: .long, help: "Window title or app name filter") var window: String
    @Option(name: .long, help: "New X position") var x: Int
    @Option(name: .long, help: "New Y position") var y: Int

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.windowMove(window: window, x: x, y: y)
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Window Close

struct AgentWindowCloseCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "window-close", abstract: "Close a window")
    @OptionGroup var connection: ConnectionOptions
    @Option(name: .long, help: "Window title or app name filter") var window: String

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.windowClose(window: window)
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Window Minimize

struct AgentWindowMinimizeCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "window-minimize", abstract: "Minimize a window")
    @OptionGroup var connection: ConnectionOptions
    @Option(name: .long, help: "Window title or app name filter") var window: String

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.windowMinimize(window: window)
        print(AgentFormatter.formatAction(response))
    }
}

// MARK: - Wait

struct AgentWaitCmd: AsyncParsableCommand {
    static let configuration = CommandConfiguration(commandName: "wait", abstract: "Wait for accessibility to be ready")
    @OptionGroup var connection: ConnectionOptions
    @Option(name: .long, help: "Window title or app name filter") var window: String?
    @Option(name: .long, help: "Timeout in seconds") var timeout: Int?

    mutating func run() async throws {
        let agent = try connection.resolveAgent()
        let response = try await agent.wait(window: window, timeout: timeout)
        print(AgentFormatter.formatAction(response))
    }
}

