// The k12 probe calls the unchanged `ProcessRunner.runShell` once and prints one
// JSON object. The runner compiles this file with `ProcessRunner.swift`, and the
// traced build also adds `ExitTrace.swift` with `-DEXIT_TRACE`.
//
// Usage: probe COMMAND TIMEOUT_SECONDS
//
// The object holds the returned fields as the agent would see them. It writes each
// string as the hex of its UTF-8 bytes, so a byte that the runner changed is
// visible. A thrown error is printed with its domain and code. The times use the
// clock of the trace seam, `CLOCK_UPTIME_RAW`, so the two can be compared.
import Foundation

func hex(_ text: String) -> String {
    text.utf8.map { String(format: "%02x", $0) }.joined()
}

func now() -> UInt64 {
    clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
}

let arguments = CommandLine.arguments
guard arguments.count == 3, let timeout = Int(arguments[2]) else {
    FileHandle.standardError.write(Data("usage: probe COMMAND TIMEOUT_SECONDS\n".utf8))
    exit(64)
}

var record: [String: Any] = ["start_ns": now()]
do {
    let result = try await ProcessRunner.runShell(command: arguments[1], timeoutSeconds: timeout)
    record["kind"] = "result"
    record["exitCode"] = Int(result.exitCode)
    record["timedOut"] = result.timedOut
    record["stdout_hex"] = hex(result.stdout)
    record["stderr_hex"] = hex(result.stderr)
} catch {
    let bridged = error as NSError
    record["kind"] = "thrown"
    record["domain"] = bridged.domain
    record["code"] = bridged.code
    record["description"] = String(describing: error)
}
record["end_ns"] = now()
let data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
FileHandle.standardOutput.write(data + Data("\n".utf8))
