import Foundation

/// QEMU / swtpm orchestration for the Windows backend.
///
/// Pure filesystem introspection lives here alongside the side-effectful
/// `start` / `stop` operations. Both mirror `scripts/macos/vm-start.sh`
/// and `vm-stop.sh` — the Swift port preserves their external behaviour
/// so a mid-migration host can use either side interchangeably.
public enum QEMURunner {

    /// Scan `$XDG_DATA_HOME/testanyware/golden/*.qcow2` for golden images.
    ///
    /// `sizeGB` is left `nil`; enriching it requires `qemu-img info` which
    /// is deferred to a follow-up (see plan note after Task 10).
    public static func scanGoldenDir(path: String) throws -> [VMListEntry] {
        guard isExistingDirectory(path) else { return [] }
        let items = try FileManager.default.contentsOfDirectory(atPath: path)
        return items
            .filter { $0.hasSuffix(".qcow2") }
            .map { item in
                let name = (item as NSString).deletingPathExtension
                return VMListEntry(
                    kind: .golden,
                    name: name,
                    platform: platformFromName(name),
                    backend: "qemu"
                )
            }
    }

    /// Scan `$XDG_DATA_HOME/testanyware/clones/*/monitor.sock` for running
    /// QEMU VMs.
    ///
    /// Only the presence of `monitor.sock` is checked — a more thorough
    /// liveness probe (monitor handshake, `lsof` on the qcow2) is left to
    /// the caller. Matches the bash script's "socket exists → list it"
    /// behaviour, though the bash also uses `lsof` to confirm a qemu
    /// process holds the image.
    public static func scanClonesDir(path: String) throws -> [VMListEntry] {
        guard isExistingDirectory(path) else { return [] }
        let items = try FileManager.default.contentsOfDirectory(atPath: path)
        var entries: [VMListEntry] = []
        for item in items {
            let cloneDir = "\(path)/\(item)"
            guard isExistingDirectory(cloneDir) else { continue }
            let sock = "\(cloneDir)/monitor.sock"
            guard FileManager.default.fileExists(atPath: sock) else { continue }
            entries.append(VMListEntry(
                kind: .running,
                name: item,
                platform: platformFromName(item),
                backend: "qemu"
            ))
        }
        return entries
    }

    public static func platformFromName(_ name: String) -> String {
        if name.contains("macos") || name.contains("tahoe") { return "macos" }
        if name.contains("linux") || name.contains("ubuntu") { return "linux" }
        if name.contains("windows") { return "windows" }
        return "unknown"
    }

    // MARK: - Lifecycle

    /// Return value of `start`. Carries the pieces `VMLifecycle` needs to
    /// populate the public spec + private meta files.
    public struct StartArtifacts: Equatable, Sendable {
        public var pid: Int
        public var vncPort: Int
        public var agentPort: Int?
        public var cloneDir: String
    }

    /// Clone the golden qcow2 + UEFI vars + TPM state, start swtpm, launch
    /// QEMU detached, and discover the dynamic VNC/agent ports via the
    /// monitor socket.
    ///
    /// Layout under `<clones>/<id>/`:
    ///   * `<id>.qcow2`              — copy-on-write overlay over the golden
    ///   * `<id>-efivars.fd`         — UEFI variable store copy
    ///   * `<id>-tpm/`               — swtpm state dir + `swtpm-sock` control socket
    ///   * `monitor.sock`            — QEMU HMP socket
    ///   * `qemu.log`                — QEMU stdout + stderr (append-mode)
    public static func start(
        options: VMStartOptions,
        paths: VMPaths
    ) async throws -> StartArtifacts {
        let cloneDir = paths.cloneDir(forID: options.id)
        let goldenDir = paths.goldenDir

        if FileManager.default.fileExists(atPath: cloneDir) {
            if let pid = processHoldingQcow2(in: cloneDir) {
                kill(pid_t(pid), SIGTERM)
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
            try FileManager.default.removeItem(atPath: cloneDir)
        }
        try FileManager.default.createDirectory(atPath: cloneDir, withIntermediateDirectories: true)

        let goldenQcow2 = "\(goldenDir)/\(options.base).qcow2"
        let cloneQcow2 = "\(cloneDir)/\(options.id).qcow2"
        try runAndCheck(
            executable: "/opt/homebrew/bin/qemu-img",
            arguments: ["create", "-f", "qcow2", "-b", goldenQcow2, "-F", "qcow2", cloneQcow2]
        )

        let goldenEfivars = "\(goldenDir)/\(options.base)-efivars.fd"
        let cloneEfivars = "\(cloneDir)/\(options.id)-efivars.fd"
        try FileManager.default.copyItem(atPath: goldenEfivars, toPath: cloneEfivars)

        let goldenTPM = "\(goldenDir)/\(options.base)-tpm"
        let cloneTPMDir = "\(cloneDir)/\(options.id)-tpm"
        try runAndCheck(
            executable: "/bin/cp",
            arguments: ["-r", goldenTPM, cloneTPMDir]
        )
        let tpmSocket = "\(cloneTPMDir)/swtpm-sock"

        let qemuPath = TartRunner.which("qemu-system-aarch64")
            ?? "/opt/homebrew/bin/qemu-system-aarch64"
        let qemuBinDir = (qemuPath as NSString).deletingLastPathComponent
        let qemuPrefix = (qemuBinDir as NSString).deletingLastPathComponent
        let uefiCode = "\(qemuPrefix)/share/qemu/edk2-aarch64-code.fd"
        guard FileManager.default.fileExists(atPath: uefiCode) else {
            try? FileManager.default.removeItem(atPath: cloneDir)
            throw QEMURunnerError.uefiNotFound(uefiCode)
        }

        try runAndCheck(
            executable: "/opt/homebrew/bin/swtpm",
            arguments: [
                "socket",
                "--tpmstate", "dir=\(cloneTPMDir)",
                "--ctrl", "type=unixio,path=\(tpmSocket)",
                "--tpm2",
                "--log", "level=0",
                "--daemon",
            ]
        )
        try? await Task.sleep(nanoseconds: 1_000_000_000)

        let monitorSock = "\(cloneDir)/monitor.sock"
        var gpuDevice = "virtio-gpu-pci"
        if let display = options.display {
            let parts = display.split(separator: "x").map(String.init)
            if parts.count == 2 {
                gpuDevice = "virtio-gpu-pci,xres=\(parts[0]),yres=\(parts[1])"
            }
        }
        let qemuArgs: [String] = [
            "-machine", "virt,highmem=on,gic-version=3",
            "-accel", "hvf",
            "-cpu", "host",
            "-smp", "4",
            "-m", "4096",
            "-drive", "if=pflash,format=raw,file=\(uefiCode),readonly=on",
            "-drive", "if=pflash,format=raw,file=\(cloneEfivars)",
            "-chardev", "socket,id=chrtpm,path=\(tpmSocket)",
            "-tpmdev", "emulator,id=tpm0,chardev=chrtpm",
            "-device", "tpm-tis-device,tpmdev=tpm0",
            "-drive", "file=\(cloneQcow2),if=none,id=hd0,format=qcow2",
            "-device", "nvme,drive=hd0,serial=boot,bootindex=0",
            "-device", "ramfb",
            "-device", gpuDevice,
            "-device", "qemu-xhci",
            "-device", "usb-kbd",
            "-device", "usb-tablet",
            "-device", "virtio-net-pci,netdev=net0",
            "-netdev", "user,id=net0,hostfwd=tcp::0-:8648",
            "-vnc", "localhost:0,to=99,password=on",
            "-monitor", "unix:\(monitorSock),server,nowait",
            "-display", "none",
        ]
        let logPath = "\(cloneDir)/qemu.log"
        let pid: Int32
        do {
            pid = try DetachedProcess.spawn(
                executable: qemuPath,
                arguments: qemuArgs,
                logPath: logPath
            )
        } catch {
            try? FileManager.default.removeItem(atPath: cloneDir)
            throw error
        }
        try? await Task.sleep(nanoseconds: 1_000_000_000)
        if kill(pid, 0) != 0 {
            try? FileManager.default.removeItem(atPath: cloneDir)
            throw QEMURunnerError.qemuFailedToStart
        }

        let client = QEMUMonitorClient(socketPath: monitorSock)
        await client.setVNCPassword("testanyware", attempts: 3)

        let agentPort = try await client.agentPort(attempts: 5, intervalSeconds: 1.0)
        guard let agentPort else {
            kill(pid, SIGTERM)
            try? FileManager.default.removeItem(atPath: cloneDir)
            throw QEMURunnerError.monitorDiscoveryFailed
        }
        let vncPort = (try? await client.vncPort(attempts: 3, intervalSeconds: 0.5)) ?? 5900

        return StartArtifacts(
            pid: Int(pid),
            vncPort: vncPort,
            agentPort: agentPort,
            cloneDir: cloneDir
        )
    }

    /// Tear down a running QEMU VM: SIGTERM the qemu pid (escalating to
    /// SIGKILL if it does not exit), best-effort kill the associated swtpm
    /// daemon, then remove the clone directory.
    ///
    /// The swtpm kill uses `pgrep -f swtpm.*<tpmDir>` rather than the legacy
    /// bash pattern (`swtpm.*<cloneDir>/tpm/sock`) because that pattern
    /// never matched the path vm-start.sh actually used. The Swift port
    /// fixes that latent bug.
    public static func stop(pid: Int, cloneDir: String) {
        if pid > 0 && kill(pid_t(pid), 0) == 0 {
            kill(pid_t(pid), SIGTERM)
            for _ in 0..<20 {
                if kill(pid_t(pid), 0) != 0 { break }
                Thread.sleep(forTimeInterval: 0.1)
            }
            if kill(pid_t(pid), 0) == 0 {
                kill(pid_t(pid), SIGKILL)
            }
        }

        // swtpm has no registry; locate it by its state-dir path and kill.
        let cloneName = (cloneDir as NSString).lastPathComponent
        let tpmDir = "\(cloneDir)/\(cloneName)-tpm"
        if let swtpmPID = pgrepFirst(pattern: "swtpm.*\(tpmDir)") {
            kill(swtpmPID, SIGTERM)
            for _ in 0..<5 {
                if kill(swtpmPID, 0) != 0 { break }
                Thread.sleep(forTimeInterval: 0.2)
            }
            if kill(swtpmPID, 0) == 0 {
                kill(swtpmPID, SIGKILL)
            }
        }

        if FileManager.default.fileExists(atPath: cloneDir) {
            try? FileManager.default.removeItem(atPath: cloneDir)
        }
    }

    // MARK: - Delete

    /// Remove a QEMU golden image's three on-disk artefacts:
    /// `<name>.qcow2`, `<name>-efivars.fd`, and the `<name>-tpm/` state
    /// directory. Missing artefacts are silently ignored (idempotent).
    public static func deleteGolden(name: String, paths: VMPaths) {
        let base = "\(paths.goldenDir)/\(name)"
        try? FileManager.default.removeItem(atPath: "\(base).qcow2")
        try? FileManager.default.removeItem(atPath: "\(base)-efivars.fd")
        try? FileManager.default.removeItem(atPath: "\(base)-tpm")
    }

    /// PIDs of running QEMU clones whose backing qcow2 is this golden
    /// image. Walks `<clonesDir>/*/*.qcow2`, reads each clone's
    /// `full-backing-filename` via `qemu-img info`, and reports the
    /// first PID that `lsof` attributes to a matching qcow2.
    public static func runningClonesBacked(
        byGoldenName name: String,
        paths: VMPaths
    ) -> [Int] {
        let goldenQcow2 = "\(paths.goldenDir)/\(name).qcow2"
        guard isExistingDirectory(paths.clonesDir) else { return [] }
        let fm = FileManager.default
        guard let items = try? fm.contentsOfDirectory(atPath: paths.clonesDir) else {
            return []
        }
        var pids: [Int] = []
        for item in items {
            let dir = "\(paths.clonesDir)/\(item)"
            guard isExistingDirectory(dir) else { continue }
            guard let subs = try? fm.contentsOfDirectory(atPath: dir) else { continue }
            for f in subs where f.hasSuffix(".qcow2") {
                let cloneFile = "\(dir)/\(f)"
                guard backingFile(ofQcow2: cloneFile) == goldenQcow2 else { continue }
                if let pid = processHoldingQcow2(in: dir) {
                    pids.append(pid)
                }
            }
        }
        return pids
    }

    /// Parse `full-backing-filename` from `qemu-img info --output=json`.
    /// Returns `nil` if qemu-img is absent, the call fails, or the JSON
    /// lacks the field — callers treat absence as "not backed by any
    /// known golden" rather than erroring.
    private static func backingFile(ofQcow2 path: String) -> String? {
        let qemuImg = TartRunner.which("qemu-img") ?? "/opt/homebrew/bin/qemu-img"
        guard FileManager.default.isExecutableFile(atPath: qemuImg) else { return nil }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: qemuImg)
        proc.arguments = ["info", "--output=json", path]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        guard proc.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        return obj["full-backing-filename"] as? String
    }

    // MARK: - Helpers

    private static func isExistingDirectory(_ path: String) -> Bool {
        var isDir: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: path, isDirectory: &isDir)
        return exists && isDir.boolValue
    }

    /// Run `executable` with `arguments`, inheriting the parent's stderr
    /// for stdout+stderr (matches bash's `qemu-img create ... >&2`).
    /// Throws on non-zero exit.
    private static func runAndCheck(executable: String, arguments: [String]) throws {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: executable)
        proc.arguments = arguments
        proc.standardOutput = FileHandle.standardError
        proc.standardError = FileHandle.standardError
        try proc.run()
        proc.waitUntilExit()
        if proc.terminationStatus != 0 {
            throw QEMURunnerError.commandFailed(
                "\(executable) \(arguments.joined(separator: " "))"
            )
        }
    }

    /// Find the first process id holding any `.qcow2` file in `dir`, via
    /// `lsof -t`. Returns `nil` on no match or lsof failure.
    private static func processHoldingQcow2(in dir: String) -> Int? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/bin/sh")
        proc.arguments = [
            "-c",
            "lsof -t \"$1\"/*.qcow2 2>/dev/null | head -1",
            "testanyware-qcow2-holder",
            dir,
        ]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let out = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return out.flatMap { Int($0) }
    }

    /// Run `pgrep -f <pattern>` and return the first matching pid.
    private static func pgrepFirst(pattern: String) -> pid_t? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
        proc.arguments = ["-f", pattern]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = Pipe()
        do { try proc.run() } catch { return nil }
        proc.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let out = String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let first = out.split(separator: "\n").first.map(String.init) ?? ""
        return Int32(first)
    }
}

public enum QEMURunnerError: Error, CustomStringConvertible, Equatable {
    case uefiNotFound(String)
    case qemuFailedToStart
    case monitorDiscoveryFailed
    case commandFailed(String)

    public var description: String {
        switch self {
        case .uefiNotFound(let path):
            return "UEFI firmware not found at \(path)"
        case .qemuFailedToStart:
            return "QEMU did not remain running after launch"
        case .monitorDiscoveryFailed:
            return "Could not discover agent port via QEMU monitor"
        case .commandFailed(let detail):
            return "Command failed: \(detail)"
        }
    }
}
