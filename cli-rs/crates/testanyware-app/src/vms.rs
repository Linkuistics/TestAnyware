//! The VMs the app serves: the listing behind `GET /v1/vms` and behind the status item's menu.
//!
//! The app lists with the lifecycle code of the vm crate, on the directories it holds and with
//! its own tool search. It keeps no list. Every request reads the sidecars and asks `tart`
//! again, so a VM that the Host CLI or an older app started appears at once (spec: "Lifecycle and
//! adoption").

use std::sync::Arc;
use std::time::Duration;

use testanyware_app_protocol::ListRequest;
use testanyware_vm::lifecycle::VmLifecycle;
use testanyware_vm::listing::{build_view, ListView};
use testanyware_vm::VmPaths;

use crate::menu::MenuVm;
use crate::AppContext;

/// The VM records of the app. The socket sessions of QEMU live under the temporary directory of
/// the app's own process, where the app starts them.
pub(crate) fn vm_paths(ctx: &AppContext) -> VmPaths {
    let directories = &ctx.identity.directories;
    VmPaths::new(
        directories.state_dir.clone(),
        directories.data_dir.clone(),
        std::env::temp_dir(),
    )
}

/// The view that `vm list` prints for `request`. It runs `tart`, so a caller on the
/// asynchronous runtime calls it from a blocking task.
pub fn list_view(ctx: &AppContext, request: &ListRequest) -> ListView {
    let listing = VmLifecycle::list_with(&vm_paths(ctx), &ctx.tools);
    build_view(&listing, &request.filters, request.limit, request.all)
}

/// The running VMs, for the menu.
fn running_vms(ctx: &AppContext) -> Vec<MenuVm> {
    let request = ListRequest {
        limit: usize::MAX,
        all: true,
        filters: vec![("kind".to_string(), "running".to_string())],
    };
    list_view(ctx, &request)
        .items
        .into_iter()
        .map(|item| MenuVm {
            id: item.name,
            platform: item.platform,
            backend: item.backend,
        })
        .collect()
}

/// The running VMs, or `None` when the listing takes longer than `limit`. A `tart` that hangs
/// must not freeze the menu, so the listing runs on its own thread. A thread that never returns
/// stays behind, and the next menu starts another.
pub fn running_vms_within(ctx: &Arc<AppContext>, limit: Duration) -> Option<Vec<MenuVm>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let ctx = Arc::clone(ctx);
    std::thread::spawn(move || {
        let _ = sender.send(running_vms(&ctx));
    });
    receiver.recv_timeout(limit).ok()
}
