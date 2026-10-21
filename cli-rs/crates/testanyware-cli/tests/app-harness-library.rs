//! The functions of `testanyware_cli::app_route` that the host harnesses call (spec: "How the
//! host harnesses reach their guests").
//!
//! Each test runs the real `testanyware-app` executable in its test mode, with a wiremock
//! In-VM agent on loopback as the guest. The library functions run in this process, so the
//! process states the directories of the app: the test sets `XDG_STATE_HOME` and
//! `XDG_DATA_HOME` for the whole process, and one test holds the serial lock.

#![cfg(unix)]

mod common;

use std::time::Duration;

use common::{serial, FakeAgent, TestApp};
use serde_json::json;
use testanyware_agent_client::AgentError;
use testanyware_app_protocol::{StreamKind, StreamTarget};
use testanyware_cli::app_route::{agent_client_with_timeout, open_guest_stream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wiremock::ResponseTemplate;

const VM: &str = "testanyware-feed0049";

/// The process states the directories that the app reads, as the Host CLI states its own.
fn state_directories_of(app: &TestApp) {
    std::env::set_var("XDG_STATE_HOME", app.xdg_state_home());
    std::env::set_var("XDG_DATA_HOME", app.xdg_data_home());
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime")
}

#[test]
fn a_guest_stream_by_vm_id_reaches_the_agent_of_that_vm() {
    let _serial = serial();
    let agent = FakeAgent::start();
    agent.mount(
        "GET",
        "/health",
        ResponseTemplate::new(200).set_body_json(json!({ "accessible": true, "platform": "linux" })),
    );
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());
    state_directories_of(&app);

    let reply = runtime().block_on(async {
        let mut stream = open_guest_stream(
            app.socket(),
            &StreamTarget::Vm { id: VM.into() },
            StreamKind::Agent,
        )
        .await
        .expect("the app opens a stream to the agent of the VM");
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: guest\r\nConnection: close\r\n\r\n")
            .await
            .expect("the request reaches the guest");
        let mut reply = Vec::new();
        stream
            .read_to_end(&mut reply)
            .await
            .expect("the guest answers");
        String::from_utf8(reply).expect("an HTTP reply in text")
    });

    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(reply.contains("\"accessible\":true"), "{reply}");
    assert_eq!(agent.request_count(), 1, "the guest saw one request");
}

#[test]
fn a_guest_stream_to_an_unknown_vm_fails_with_the_error_of_the_app() {
    let _serial = serial();
    let app = TestApp::start();
    state_directories_of(&app);

    let error = runtime()
        .block_on(open_guest_stream(
            app.socket(),
            &StreamTarget::Vm { id: "testanyware-0000dead".into() },
            StreamKind::Agent,
        ))
        .expect_err("an id with no spec has no stream");

    assert_eq!(error.code, "VM_NOT_FOUND");
}

#[test]
fn the_caller_sets_the_timeout_of_each_request() {
    let _serial = serial();
    let agent = FakeAgent::start();
    agent.mount(
        "GET",
        "/health",
        ResponseTemplate::new(200)
            .set_body_json(json!({ "accessible": true, "platform": "linux" }))
            .set_delay(Duration::from_millis(1500)),
    );
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());
    state_directories_of(&app);
    let target = || StreamTarget::Vm { id: VM.into() };

    let short = agent_client_with_timeout(app.socket(), target(), Duration::from_millis(300));
    let error = runtime()
        .block_on(short.health())
        .expect_err("the reply comes after the timeout");
    assert!(matches!(error, AgentError::Timeout(_)), "{error:?}");

    let long = agent_client_with_timeout(app.socket(), target(), Duration::from_secs(30));
    let health = runtime()
        .block_on(long.health())
        .expect("the reply comes inside the timeout");
    assert!(health.accessible);
}
