import Foundation

/// Polls the in-VM agent's `/health` endpoint until a 2xx arrives or the
/// attempt budget is exhausted.
///
/// This is the Swift successor to the SSH-readiness / HTTP-readiness loops
/// in `scripts/macos/vm-start.sh`. Used by `VMLifecycle.start` to decide
/// whether to populate the `agent` field on the spec file.
public struct AgentHealthWaiter: Sendable {

    public init() {}

    /// Poll `http://<host>:<port>/health` up to `attempts` times, waiting
    /// `intervalSeconds` between attempts.
    ///
    /// Returns `true` on the first 2xx response, `false` if the budget is
    /// exhausted without one. Connection failures and non-2xx responses
    /// are treated uniformly as "not ready yet."
    public func waitForReady(
        host: String,
        port: Int,
        attempts: Int,
        intervalSeconds: Double
    ) async throws -> Bool {
        let session = URLSession(configuration: .ephemeral)
        guard let url = URL(string: "http://\(host):\(port)/health") else {
            return false
        }

        for attempt in 0..<attempts {
            var request = URLRequest(url: url, timeoutInterval: 2)
            request.httpMethod = "GET"
            do {
                let (_, response) = try await session.data(for: request)
                if let http = response as? HTTPURLResponse,
                   (200..<300).contains(http.statusCode) {
                    return true
                }
            } catch {
                // connection refused / timeout — fall through and retry
            }

            let isLastAttempt = attempt == attempts - 1
            if !isLastAttempt {
                try await Task.sleep(nanoseconds: UInt64(intervalSeconds * 1_000_000_000))
            }
        }
        return false
    }
}
