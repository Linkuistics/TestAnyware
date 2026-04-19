import Foundation

/// End-to-end VM lifecycle orchestrator.
///
/// Composes the per-backend runners (`TartRunner`, `QEMURunner`), health
/// waiters (`AgentHealthWaiter`), and optional UI viewer (`VNCViewer`)
/// into the `testanyware vm start` / `vm stop` flows. Persists the public
/// spec (`VMSpec`) and private sidecar (`VMMeta`) on success so later CLI
/// invocations can locate and tear down the VM.
public enum VMLifecycle {

    /// Start a VM end-to-end and return the generated id, spec, and meta.
    /// Callers print `result.id` to stdout.
    public static func start(options: VMStartOptions) async throws -> VMStartResult {
        switch options.platform.backend {
        case .tart:
            return try await startTart(options: options)
        case .qemu:
            return try await startQEMU(options: options)
        }
    }

    /// Stop a VM, close any associated viewer windows, and remove the
    /// public spec + private meta sidecar. Best-effort: a missing or
    /// already-stopped VM throws `.notFound`; partial cleanup is surfaced
    /// via `.stopFailed` after removing whatever files could be removed.
    public static func stop(id: String) throws {
        let paths = VMPaths()
        let specPath = paths.specPath(forID: id)
        let metaPath = paths.metaPath(forID: id)

        let meta = (try? VMMeta.load(from: metaPath))
            ?? VMMeta(id: id, tool: .tart, pid: 0, cloneDir: nil, viewerWindowID: nil)
        let spec = try? VMSpec.load(from: specPath)
        let vncPort = spec?.vnc.port

        if VNCViewer.tccPreflight() {
            if meta.viewerWindowID != nil {
                logStderr("Closing tracked VNC viewer window...")
            }
            if let port = vncPort {
                logStderr("Closing any remaining VNC viewers connected to port \(port)...")
            }
            VNCViewer.closeWindows(identifier: meta.viewerWindowID, vncPort: vncPort)
        }

        logStderr("Stopping VM '\(id)'...")
        var ok = true
        switch meta.tool {
        case .tart:
            let entries = (try? TartRunner.runList()) ?? []
            if entries.contains(where: { $0.name == id }) {
                TartRunner.removeExisting(id: id)
                let after = (try? TartRunner.runList()) ?? []
                if after.contains(where: { $0.name == id }) {
                    logStderr("ERROR: failed to delete tart VM '\(id)'.")
                    ok = false
                }
            } else {
                logStderr("ERROR: tart VM '\(id)' does not exist.")
                ok = false
            }
            if meta.pid > 0 && kill(pid_t(meta.pid), 0) == 0 {
                kill(pid_t(meta.pid), SIGTERM)
            }
        case .qemu:
            if let cloneDir = meta.cloneDir, !cloneDir.isEmpty {
                QEMURunner.stop(pid: meta.pid, cloneDir: cloneDir)
            } else {
                logStderr("ERROR: meta sidecar for qemu VM '\(id)' missing clone_dir; nothing to tear down.")
                ok = false
            }
        }

        try? FileManager.default.removeItem(atPath: specPath)
        try? FileManager.default.removeItem(atPath: metaPath)

        if ok {
            logStderr("VM '\(id)' stopped and deleted.")
        } else {
            logStderr("VM '\(id)' cleanup completed with errors.")
            throw VMLifecycleError.stopFailed(id: id)
        }
    }

    /// Delete a golden image by name. Auto-detects the backend: if the
    /// name appears in `tart list`, route to `TartRunner.deleteGolden`;
    /// otherwise look for a `.qcow2` in the QEMU golden directory.
    /// Refuses to proceed when running clones appear to depend on the
    /// image, unless `force` is `true`.
    public static func delete(name: String, force: Bool) throws {
        let paths = VMPaths()

        let tartEntries = (try? TartRunner.runList()) ?? []
        let tartExists = tartEntries.contains { $0.name == name && $0.kind == .golden }
        let qemuQcow2 = "\(paths.goldenDir)/\(name).qcow2"
        let qemuExists = FileManager.default.fileExists(atPath: qemuQcow2)

        if !tartExists && !qemuExists {
            logStderr("ERROR: Golden image '\(name)' not found in tart or QEMU.")
            logStderr("Run testanyware vm list to see available images.")
            throw VMLifecycleError.goldenNotFound(name)
        }

        let backend: VMBackend = tartExists ? .tart : .qemu
        logStderr("Found '\(name)' (backend: \(backend.rawValue))")

        switch backend {
        case .qemu:
            if !force {
                let pids = QEMURunner.runningClonesBacked(byGoldenName: name, paths: paths)
                if !pids.isEmpty {
                    logStderr("ERROR: Running clones (PIDs \(pids)) are backed by this image.")
                    logStderr("Stop the clones first, or use --force to delete anyway.")
                    throw VMLifecycleError.runningClonesPresent(name: name, pids: pids)
                }
            }
            logStderr("Deleting QEMU golden image '\(name)'...")
            QEMURunner.deleteGolden(name: name, paths: paths)
        case .tart:
            if !force {
                let running = TartRunner.runningClones()
                if !running.isEmpty {
                    logStderr("WARNING: Running tart clones \(running) may be based on this image.")
                    logStderr("Use --force to delete anyway.")
                    throw VMLifecycleError.runningClonesPresent(name: name, pids: [])
                }
            }
            logStderr("Deleting tart image '\(name)'...")
            if !TartRunner.deleteGolden(name: name) {
                throw VMLifecycleError.tartDeleteFailed(name)
            }
        }

        logStderr("Deleted '\(name)'.")
    }

    // MARK: - tart

    private static func startTart(options: VMStartOptions) async throws -> VMStartResult {
        let paths = VMPaths()
        try FileManager.default.createDirectory(
            atPath: paths.vmsDir,
            withIntermediateDirectories: true
        )

        if tartVMExists(id: options.id) {
            logStderr("Stopping existing VM '\(options.id)'...")
            TartRunner.removeExisting(id: options.id)
            try? await Task.sleep(nanoseconds: 2_000_000_000)
        }

        logStderr("Cloning \(options.base) → \(options.id)...")
        try TartRunner.clone(base: options.base, id: options.id)

        if let display = options.display {
            logStderr("Setting display to \(display)...")
            try TartRunner.setDisplay(id: options.id, display: display)
        }

        let (pid, logPath) = try TartRunner.runDetached(id: options.id, logDir: paths.vmsDir)
        logStderr("tart PID: \(pid)")

        logStderr("Waiting for VNC...")
        guard let vncURL = TartRunner.pollVNCURL(
            logPath: logPath,
            attempts: 60,
            intervalSeconds: 1.0
        ) else {
            logStderr("ERROR: VM did not produce a VNC URL within 60s")
            kill(pid, SIGTERM)
            throw VMLifecycleError.vncTimeout
        }
        logStderr("VNC: \(vncURL.host):\(vncURL.port)")

        logStderr("Waiting for VM IP...")
        let ip = TartRunner.pollIP(id: options.id, attempts: 30, intervalSeconds: 2.0)
        if ip == nil {
            logStderr("WARNING: Could not get VM IP — agent and SSH will be unavailable")
        }

        // SSH wait — temporary; dies with backlog Task 5 (SSH disabled in goldens).
        var sshEndpoint: String? = nil
        if let ipValue = ip {
            logStderr("Waiting for SSH at admin@\(ipValue)...")
            if sshReady(ip: ipValue, attempts: 40, intervalSeconds: 3.0) {
                sshEndpoint = "admin@\(ipValue)"
                logStderr("SSH: \(sshEndpoint!) (debug convenience)")
            } else {
                logStderr("WARNING: SSH not reachable — SSH debugging will be unavailable")
            }
        }

        var agentEndpoint: AgentSpec? = nil
        if let ipValue = ip {
            logStderrInline("Waiting for agent at \(ipValue):8648...")
            let waiter = AgentHealthWaiter()
            let ready = (try? await waiter.waitForReady(
                host: ipValue,
                port: 8648,
                attempts: 60,
                intervalSeconds: 2.0
            )) ?? false
            if ready {
                agentEndpoint = AgentSpec(host: ipValue, port: 8648)
                logStderr(" ready.")
            } else {
                logStderr("")
                logStderr("WARNING: Agent not reachable at \(ipValue):8648 — agent commands will fail")
            }
        }

        var viewerID: String? = nil
        if options.openViewer {
            if VNCViewer.tccPreflight() {
                logStderr("Opening VNC viewer...")
                let urlString = "vnc://:\(vncURL.password ?? "")@\(vncURL.host):\(vncURL.port)"
                viewerID = VNCViewer.openAndCapture(vncURL: urlString, password: vncURL.password)
            } else {
                logStderr(
                    "Automation permission required. Grant 'testanyware' access to " +
                    "System Events under Privacy & Security → Automation, or re-run without --viewer."
                )
            }
        }

        let spec = VMSpec(
            vnc: VNCSpec(host: vncURL.host, port: vncURL.port, password: vncURL.password),
            agent: agentEndpoint,
            platform: options.platform,
            ssh: sshEndpoint
        )
        try spec.writeAtomic(to: paths.specPath(forID: options.id))

        let meta = VMMeta(
            id: options.id,
            tool: .tart,
            pid: Int(pid),
            cloneDir: nil,
            viewerWindowID: viewerID
        )
        try meta.writeAtomic(to: paths.metaPath(forID: options.id))

        logStderr("")
        logStderr("VM ready.")
        logStderr("  Spec:   \(paths.specPath(forID: options.id))")
        logStderr("  Meta:   \(paths.metaPath(forID: options.id))")
        if let a = agentEndpoint { logStderr("  Agent:  \(a.host):\(a.port)") }
        if let s = sshEndpoint { logStderr("  SSH:    \(s)") }
        logStderr("")
        logStderr("Use with the CLI:")
        logStderr("  testanyware screenshot --vm \(options.id) -o s.png")
        logStderr("  export TESTANYWARE_VM_ID=\(options.id)      # then --vm becomes optional")
        logStderr("")
        logStderr("Stop the VM:")
        logStderr("  testanyware vm stop \(options.id)")
        logStderr("")

        return VMStartResult(id: options.id, spec: spec, meta: meta)
    }

    // MARK: - qemu

    private static func startQEMU(options: VMStartOptions) async throws -> VMStartResult {
        let paths = VMPaths()
        try FileManager.default.createDirectory(
            atPath: paths.vmsDir,
            withIntermediateDirectories: true
        )

        logStderr("Cloning \(options.base) → \(options.id)...")
        let artifacts = try await QEMURunner.start(options: options, paths: paths)

        logStderr("qemu PID: \(artifacts.pid)")
        logStderr("VNC: localhost:\(artifacts.vncPort)")
        if let port = artifacts.agentPort {
            logStderr("Agent port: \(port)")
        }

        var agentEndpoint: AgentSpec? = nil
        if let port = artifacts.agentPort {
            logStderrInline("Waiting for agent at localhost:\(port)...")
            let waiter = AgentHealthWaiter()
            let ready = (try? await waiter.waitForReady(
                host: "localhost",
                port: port,
                attempts: 120,
                intervalSeconds: 5.0
            )) ?? false
            if ready {
                agentEndpoint = AgentSpec(host: "localhost", port: port)
                logStderr(" ready.")
            } else {
                logStderr("")
                logStderr("WARNING: Agent not reachable at localhost:\(port) — agent commands will fail")
            }
        }

        var viewerID: String? = nil
        if options.openViewer {
            if VNCViewer.tccPreflight() {
                logStderr("Opening VNC viewer...")
                let url = "vnc://:testanyware@localhost:\(artifacts.vncPort)"
                viewerID = VNCViewer.openAndCapture(vncURL: url, password: "testanyware")
            } else {
                logStderr(
                    "Automation permission required. Grant 'testanyware' access to " +
                    "System Events under Privacy & Security → Automation, or re-run without --viewer."
                )
            }
        }

        let spec = VMSpec(
            vnc: VNCSpec(host: "localhost", port: artifacts.vncPort, password: "testanyware"),
            agent: agentEndpoint,
            platform: options.platform,
            ssh: nil
        )
        try spec.writeAtomic(to: paths.specPath(forID: options.id))

        let meta = VMMeta(
            id: options.id,
            tool: .qemu,
            pid: artifacts.pid,
            cloneDir: artifacts.cloneDir,
            viewerWindowID: viewerID
        )
        try meta.writeAtomic(to: paths.metaPath(forID: options.id))

        logStderr("")
        logStderr("VM ready.")
        logStderr("  Spec:   \(paths.specPath(forID: options.id))")
        logStderr("  Meta:   \(paths.metaPath(forID: options.id))")
        if let a = agentEndpoint { logStderr("  Agent:  \(a.host):\(a.port)") }
        logStderr("")
        logStderr("Use with the CLI:")
        logStderr("  testanyware screenshot --vm \(options.id) -o s.png")
        logStderr("  export TESTANYWARE_VM_ID=\(options.id)      # then --vm becomes optional")
        logStderr("")
        logStderr("Stop the VM:")
        logStderr("  testanyware vm stop \(options.id)")
        logStderr("")

        return VMStartResult(id: options.id, spec: spec, meta: meta)
    }

    private static func tartVMExists(id: String) -> Bool {
        let entries = (try? TartRunner.runList()) ?? []
        return entries.contains { $0.name == id }
    }

    private static func sshReady(ip: String, attempts: Int, intervalSeconds: Double) -> Bool {
        for attempt in 0..<attempts {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/usr/bin/ssh")
            proc.arguments = [
                "-o", "StrictHostKeyChecking=no",
                "-o", "UserKnownHostsFile=/dev/null",
                "-o", "LogLevel=ERROR",
                "-o", "ConnectTimeout=5",
                "-o", "BatchMode=yes",
                "admin@\(ip)", "echo ok"
            ]
            proc.standardOutput = Pipe()
            proc.standardError = Pipe()
            do { try proc.run() } catch { return false }
            proc.waitUntilExit()
            if proc.terminationStatus == 0 { return true }
            if attempt < attempts - 1 {
                Thread.sleep(forTimeInterval: intervalSeconds)
            }
        }
        return false
    }

    private static func logStderr(_ msg: String) {
        FileHandle.standardError.write(Data((msg + "\n").utf8))
    }

    private static func logStderrInline(_ msg: String) {
        FileHandle.standardError.write(Data(msg.utf8))
    }
}

public enum VMLifecycleError: Error, CustomStringConvertible, Equatable {
    case vncTimeout
    case unsupportedBackend(String)
    case stopFailed(id: String)
    case goldenNotFound(String)
    case runningClonesPresent(name: String, pids: [Int])
    case tartDeleteFailed(String)

    public var description: String {
        switch self {
        case .vncTimeout:
            return "VM did not produce a VNC URL in time"
        case .unsupportedBackend(let m):
            return m
        case .stopFailed(let id):
            return "VM '\(id)' stop failed"
        case .goldenNotFound(let name):
            return "Golden image '\(name)' not found in tart or QEMU"
        case .runningClonesPresent(let name, let pids):
            if pids.isEmpty {
                return "Running clones may depend on '\(name)'; re-run with --force to override"
            }
            return "Running clones (PIDs \(pids)) are backed by '\(name)'; re-run with --force to override"
        case .tartDeleteFailed(let name):
            return "tart delete '\(name)' failed"
        }
    }
}
