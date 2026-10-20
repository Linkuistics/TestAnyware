//! What one `vm start` runs on: the directories, the tools, where it reports, and how it ends.
//!
//! The Host CLI starts a VM for itself and ends with the VM started. The TestAnyware app starts
//! a VM for a client that may leave, and it lives on after the start. The context carries the
//! difference, so one start code serves both:
//!
//! | | Host CLI | TestAnyware app |
//! |---|---|---|
//! | tools | `PATH` of the process | the built-in search |
//! | progress | stderr | the events of the response |
//! | end | a signal during `--wait` | the client closes its connection |

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::paths::VmPaths;
use crate::tools::ToolSearch;

/// The port of the In-VM agent in a tart guest.
pub const AGENT_PORT: u16 = 8648;

/// Where a start reports what it does: the lines that `vm start` prints to stderr. A line is
/// progress, a warning, or the name of a wait, and it never carries a result.
#[derive(Clone)]
pub struct Progress(Arc<dyn Fn(&str) + Send + Sync>);

impl Progress {
    /// Print each line to stderr, as `vm start` always has.
    pub fn stderr() -> Self {
        Self(Arc::new(|line| eprintln!("{line}")))
    }

    /// Hand each line to `sink`.
    pub fn to(sink: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self(Arc::new(sink))
    }

    pub fn line(&self, line: &str) {
        (self.0)(line);
    }
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Progress")
    }
}

/// Everything that a start reads from the process around it.
#[derive(Debug, Clone)]
pub struct StartContext {
    pub paths: VmPaths,
    /// Where the start looks for `tart`, QEMU and `swtpm`.
    pub tools: ToolSearch,
    pub progress: Progress,
    /// Cancelling it ends the start at its next wait and cleans up as after a failed start. A
    /// start that has succeeded is stopped again.
    pub cancel: CancellationToken,
    /// The port of the agent in the guest. A test moves it to a fake agent, or to a closed
    /// port that starves the probes of a real start.
    pub agent_port: u16,
    /// Whether SIGINT and SIGTERM during a `--wait` start remove the clone and end the process.
    /// The Host CLI does so. The app must not: a handler there would end the app.
    pub process_signals: bool,
}

impl StartContext {
    /// The Host CLI starting a VM for itself.
    pub fn direct(paths: VmPaths) -> Self {
        Self {
            paths,
            tools: ToolSearch::from_process_path(),
            progress: Progress::stderr(),
            cancel: CancellationToken::new(),
            agent_port: AGENT_PORT,
            process_signals: true,
        }
    }

    /// The app starting a VM for a client. `cancel` fires when the client goes away.
    pub fn for_app(
        paths: VmPaths,
        tools: ToolSearch,
        progress: Progress,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            paths,
            tools,
            progress,
            cancel,
            agent_port: AGENT_PORT,
            process_signals: false,
        }
    }

    pub fn with_agent_port(mut self, port: u16) -> Self {
        self.agent_port = port;
        self
    }
}
