// Review k233 R2 old-path probe: an accepted timeout overflows after spawn.
import Foundation

let timeout = Int(CommandLine.arguments[1])!
let done = DispatchSemaphore(value: 0)
Task {
    let result = await ProcessRunner.runOwned(command: "printf ok", timeoutSeconds: timeout)
    print("outcome \(result.outcome) stdout \(result.legacy.stdout)")
    done.signal()
}
done.wait()
