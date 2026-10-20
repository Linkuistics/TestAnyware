//! The AppKit shell: one status item, its menu and the login item (spec: "The TestAnyware app",
//! "How the app starts"). The shell owns the main thread. The server runs on the asynchronous
//! runtime on its own threads (ADR-0005), and the shell reads its state only when the menu opens.
//!
//! The shell makes no decision. `menu::build_menu` decides what the menu shows, and this module
//! turns the entries into menu items and the clicks into calls.

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSControlStateValueOn,
    NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem, NSWorkspace,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSString, NSURL,
};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

use crate::menu::{
    build_menu, LocalNetworkState, LoginItemStatus, MenuAction, MenuEntry, MenuState,
    LOCAL_NETWORK_SETTINGS_URL,
};
use crate::vms::running_vms_within;
use crate::AppContext;

/// How long a menu waits for the list of running VMs before it says there is none.
const LISTING_PATIENCE: std::time::Duration = std::time::Duration::from_secs(2);

/// What the shell reads from the rest of the app.
pub struct ShellConfig {
    pub ctx: Arc<AppContext>,
    /// True while the server accepts connections on the socket.
    pub serving: Arc<AtomicBool>,
    /// The socket file. A quit removes it, so no stale file stays behind.
    pub socket: PathBuf,
}

struct DelegateIvars {
    config: ShellConfig,
    // The status item and the menu live as long as the app does.
    status_item: OnceCell<Retained<NSStatusItem>>,
    menu: OnceCell<Retained<NSMenu>>,
    /// The error text of the last failed `register()` or `unregister()`.
    login_problem: RefCell<Option<String>>,
}

define_class!(
    // SAFETY:
    // - The superclass NSObject does not have any subclassing requirements.
    // - `Delegate` does not implement `Drop`.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    struct Delegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for Delegate {}

    // SAFETY: `NSApplicationDelegate` has no safety requirements.
    unsafe impl NSApplicationDelegate for Delegate {
        // SAFETY: The signature is correct.
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            self.install_status_item();
        }

        // SAFETY: The signature is correct.
        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            // A clean quit leaves no socket file. A killed app leaves one, and the next start
            // removes it.
            let _ = std::fs::remove_file(&self.ivars().config.socket);
        }
    }

    // SAFETY: `NSMenuDelegate` has no safety requirements.
    unsafe impl NSMenuDelegate for Delegate {
        // The menu is built again each time it opens, so it shows the state of that moment.
        // SAFETY: The signature is correct.
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            self.rebuild(menu);
        }
    }

    impl Delegate {
        // SAFETY: Each action has the signature of a menu item's action.
        #[unsafe(method(openLocalNetworkSettings:))]
        fn open_local_network_settings(&self, _sender: Option<&AnyObject>) {
            open_url(LOCAL_NETWORK_SETTINGS_URL);
        }

        #[unsafe(method(toggleStartAtLogin:))]
        fn toggle_start_at_login(&self, _sender: Option<&AnyObject>) {
            self.toggle_login_item();
        }

        #[unsafe(method(openLoginItemsSettings:))]
        fn open_login_items_settings(&self, _sender: Option<&AnyObject>) {
            // https://developer.apple.com/documentation/servicemanagement/smappservice/opensystemsettingsloginitems()
            unsafe { SMAppService::openSystemSettingsLoginItems() };
        }

        #[unsafe(method(quitApp:))]
        fn quit_app(&self, _sender: Option<&AnyObject>) {
            // `terminate:` runs `applicationWillTerminate:`. The VMs are other processes and
            // keep running.
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, config: ShellConfig) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars {
            config,
            status_item: OnceCell::new(),
            menu: OnceCell::new(),
            login_problem: RefCell::new(None),
        });
        // SAFETY: The signature of `NSObject`'s `init` method is correct.
        unsafe { msg_send![super(this), init] }
    }

    fn install_status_item(&self) {
        let mtm = self.mtm();
        // https://developer.apple.com/documentation/appkit/nsstatusbar/statusitem(withlength:)
        let status_item = NSStatusBar::systemStatusBar()
            .statusItemWithLength(objc2_app_kit::NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            let symbol = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                ns_string!("display"),
                Some(ns_string!("TestAnyware")),
            );
            match symbol {
                Some(image) => button.setImage(Some(&image)),
                // A system without the symbol still gets a visible item.
                None => button.setTitle(ns_string!("TA")),
            }
        }
        let menu = NSMenu::new(mtm);
        // Items without an action stay disabled, and that is how the text lines look.
        menu.setAutoenablesItems(true);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        self.rebuild(&menu);
        status_item.setMenu(Some(&menu));
        let _ = self.ivars().status_item.set(status_item);
        let _ = self.ivars().menu.set(menu);
    }

    fn state(&self) -> MenuState {
        let config = &self.ivars().config;
        let grant = config
            .ctx
            .grant
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        MenuState {
            version: config.ctx.identity.build.clone(),
            serving: config.serving.load(Ordering::SeqCst),
            local_network: LocalNetworkState::from(&*grant),
            running_vms: running_vms_within(&config.ctx, LISTING_PATIENCE),
            login: login_item_status(),
            login_problem: self.ivars().login_problem.borrow().clone(),
        }
    }

    fn rebuild(&self, menu: &NSMenu) {
        let mtm = self.mtm();
        menu.removeAllItems();
        for entry in build_menu(&self.state()) {
            match entry {
                MenuEntry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
                MenuEntry::Text(text) => {
                    let item = self.item(&text, None);
                    menu.addItem(&item);
                }
                MenuEntry::Action { title, action } => {
                    let item = self.item(&title, Some(selector(action)));
                    menu.addItem(&item);
                }
                MenuEntry::Switch { title, on, action } => {
                    let item = self.item(&title, Some(selector(action)));
                    if on {
                        item.setState(NSControlStateValueOn);
                    }
                    menu.addItem(&item);
                }
            }
        }
    }

    fn item(&self, title: &str, action: Option<Sel>) -> Retained<NSMenuItem> {
        // SAFETY: `action` is a selector that `Delegate` implements, and the target is the
        // delegate, which lives as long as the app.
        unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                self.mtm().alloc(),
                &NSString::from_str(title),
                action,
                ns_string!(""),
            );
            if action.is_some() {
                item.setTarget(Some(self));
            }
            item
        }
    }

    /// Register the app as a login item, or unregister it. The status is read back afterwards,
    /// and nothing remembers what the person asked for.
    /// https://developer.apple.com/documentation/servicemanagement/smappservice/register()
    /// https://developer.apple.com/documentation/servicemanagement/smappservice/unregister()
    fn toggle_login_item(&self) {
        // https://developer.apple.com/documentation/servicemanagement/smappservice/mainapp
        let service = unsafe { SMAppService::mainAppService() };
        let result = if login_item_status().is_registered() {
            unsafe { service.unregisterAndReturnError() }
        } else {
            unsafe { service.registerAndReturnError() }
        };
        *self.ivars().login_problem.borrow_mut() =
            result.err().map(|e| e.localizedDescription().to_string());
    }
}

fn selector(action: MenuAction) -> Sel {
    match action {
        MenuAction::OpenLocalNetworkSettings => sel!(openLocalNetworkSettings:),
        MenuAction::ToggleStartAtLogin => sel!(toggleStartAtLogin:),
        MenuAction::OpenLoginItemsSettings => sel!(openLoginItemsSettings:),
        MenuAction::Quit => sel!(quitApp:),
    }
}

fn login_item_status() -> LoginItemStatus {
    // SAFETY: `mainAppService` and `status` take no arguments.
    let status = unsafe { SMAppService::mainAppService().status() };
    match status {
        SMAppServiceStatus::Enabled => LoginItemStatus::Enabled,
        SMAppServiceStatus::RequiresApproval => LoginItemStatus::RequiresApproval,
        SMAppServiceStatus::NotFound => LoginItemStatus::NotFound,
        _ => LoginItemStatus::NotRegistered,
    }
}

fn open_url(url: &str) {
    if let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) {
        NSWorkspace::sharedWorkspace().openURL(&url);
    }
}

/// Run the AppKit loop on the calling thread until the app quits. The caller must be the main
/// thread, and it must have started the server before this call.
pub fn run(config: ShellConfig) {
    let mtm = MainThreadMarker::new().expect("the AppKit loop runs on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // The bundle's `LSUIElement` gives the same policy. This covers a run outside a bundle.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let delegate = Delegate::new(mtm, config);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}
