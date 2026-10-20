//! Routing of agent connections through the TestAnyware app (spec: "The protocol").
//!
//! With `TESTANYWARE_APP_SOCKET` set, every command that uses the agent client gets each
//! connection from the app as a guest stream, and the Host CLI dials nothing itself. The
//! agent client runs its HTTP/1.1 on that stream as it runs it on a TCP connection, so the
//! output of a command does not change.

use std::path::PathBuf;
use std::sync::Arc;

use testanyware_agent_client::{
    AgentClient, AgentConfig, AgentConnector, BoxedStream, ConnectFuture, ConnectorFailure,
};
use testanyware_app_client::AppClient;
use testanyware_app_protocol::version::SUPPORTED_RANGE;
use testanyware_app_protocol::{AppError, ClientHello, Directories, StreamKind, StreamTarget};
use testanyware_vm::paths::VmPaths;

use crate::resolve::ResolvedAgent;

/// What each request states about this Host CLI: the protocol range it speaks, its build, and
/// the directories it resolved. The app reads the VM records in the state directory, so the
/// directory that goes here is the one the Host CLI reads its own records from.
pub fn client_hello() -> ClientHello {
    let paths = VmPaths::from_process_env();
    ClientHello::new(
        SUPPORTED_RANGE,
        env!("CARGO_PKG_VERSION"),
        Directories {
            state_dir: paths.state_dir().to_path_buf(),
            data_dir: paths.data_dir().to_path_buf(),
        },
    )
}

/// An agent client whose every connection is a guest stream of the app on `socket`.
pub fn agent_client(socket: PathBuf, target: StreamTarget) -> AgentClient {
    // The config only names the agent in the `Host` header and in messages. A VM is named by
    // its id, because the Host CLI does not read its spec on this route.
    let (host, port) = match &target {
        StreamTarget::Vm { id } => (id.clone(), ResolvedAgent::DEFAULT_PORT),
        StreamTarget::Endpoint { host, port } => (host.clone(), *port),
    };
    let connector = AppAgentConnector {
        client: AppClient::new(socket, client_hello()),
        target,
    };
    AgentClient::with_connector(AgentConfig::new(host, port), Arc::new(connector))
}

/// Gets each agent connection from the app.
struct AppAgentConnector {
    client: AppClient,
    target: StreamTarget,
}

impl AgentConnector for AppAgentConnector {
    fn connect(&self) -> ConnectFuture {
        let client = self.client.clone();
        let target = self.target.clone();
        Box::pin(async move {
            client
                .open_stream(&target, StreamKind::Agent)
                .await
                .map(|stream| Box::new(stream) as BoxedStream)
                .map_err(|error| failure_of(error).into())
        })
    }
}

/// The app's error object as the failure that the agent client hands on. The Host CLI prints
/// it in its own envelope, so the object keeps its code, message, remedy and details.
fn failure_of(error: AppError) -> ConnectorFailure {
    ConnectorFailure {
        code: error.code,
        message: error.message,
        remediation: error.remediation,
        details: error.details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_error_object_keeps_every_field() {
        let failure = failure_of(AppError::new(
            "CONNECTION_REFUSED",
            "m",
            Some("r".into()),
            json!({ "errno": 61, "cause": "host_unreachable" }),
        ));
        assert_eq!(failure.code, "CONNECTION_REFUSED");
        assert_eq!(failure.message, "m");
        assert_eq!(failure.remediation.as_deref(), Some("r"));
        assert_eq!(failure.details["errno"], 61);
    }

    #[test]
    fn the_hello_states_the_range_the_build_and_the_directories() {
        let hello = client_hello();
        assert_eq!(hello.protocol_range, Some(SUPPORTED_RANGE));
        assert_eq!(hello.build.as_deref(), Some(env!("CARGO_PKG_VERSION")));
        let dirs = hello.directories.expect("directories");
        assert!(dirs.state_dir.ends_with("testanyware"));
        assert!(dirs.data_dir.ends_with("testanyware"));
    }
}
