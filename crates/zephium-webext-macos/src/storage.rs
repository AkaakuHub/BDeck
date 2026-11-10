use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::Message;
use objc2_foundation::{NSError, NSString};
use objc2_web_kit::{WKWebExtensionContext, WKWebExtensionMessagePort};
use serde_json::{json, Value};
use zephium_webext::storage::{
    broker::{Changed, Client},
    Area, Context, Operation,
};

use crate::{json, runtime::Shared};

pub(crate) const APPLICATION: &str = "net.akaaku.bdeck.storage";
static NEXT_PORT: AtomicUsize = AtomicUsize::new(1);
const MAX_PORTS: usize = 128;
const MAX_PENDING: usize = 8;

thread_local! {
    static PORTS: RefCell<HashMap<usize, Rc<Connection>>> = RefCell::new(HashMap::new());
}

struct Connection {
    key: usize,
    owner: Weak<Shared>,
    context: usize,
    client: Client,
    port: Retained<WKWebExtensionMessagePort>,
    pending: RefCell<HashSet<u64>>,
    closed: Cell<bool>,
}

pub(crate) fn connect(
    shared: &Rc<Shared>,
    context: &WKWebExtensionContext,
    port: &WKWebExtensionMessagePort,
) -> Result<(), String> {
    let extension = unsafe { context.uniqueIdentifier() }.to_string();
    let allowed = shared
        .loaded
        .borrow()
        .get(&extension)
        .is_some_and(|loaded| {
            std::ptr::eq(&*loaded.context, context)
                && unsafe { context.hasPermission(&NSString::from_str("storage")) }
        });
    if !allowed {
        return Err("The extension does not have storage permission.".into());
    }
    let client = shared
        .host()
        .storage_client(&extension)
        .ok_or("Shared extension storage is unavailable.")?;
    if PORTS.with(|ports| ports.borrow().len()) >= MAX_PORTS {
        return Err("Too many extension storage connections.".into());
    }
    let key = NEXT_PORT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |key| {
            key.checked_add(1)
        })
        .map_err(|_| "Extension storage connection identity exhausted.")?;
    let connection = Rc::new(Connection {
        key,
        owner: Rc::downgrade(shared),
        context: context as *const _ as usize,
        client,
        port: port.retain(),
        pending: RefCell::new(HashSet::new()),
        closed: Cell::new(false),
    });
    let weak = Rc::downgrade(&connection);
    let message = RcBlock::new(move |message: *mut AnyObject, _error: *mut NSError| {
        if let Some(connection) = weak.upgrade() {
            connection.receive(json::from_object(unsafe { message.as_ref() }));
        }
    });
    let weak = Rc::downgrade(&connection);
    let disconnected = RcBlock::new(move |_error: *mut NSError| {
        if let Some(connection) = weak.upgrade() {
            connection.close();
        }
    });
    unsafe {
        port.setMessageHandler(Some(&message));
        port.setDisconnectHandler(Some(&disconnected));
    }
    PORTS.with(|ports| ports.borrow_mut().insert(key, connection));
    Ok(())
}

impl Connection {
    fn active(&self) -> bool {
        !self.closed.get()
            && self.owner.upgrade().is_some_and(|shared| {
                shared
                    .loaded
                    .borrow()
                    .get(self.client.installation.extension.as_str())
                    .is_some_and(|loaded| {
                        Retained::as_ptr(&loaded.context) as usize == self.context
                            && unsafe {
                                loaded.context.hasPermission(&NSString::from_str("storage"))
                            }
                    })
            })
    }

    fn receive(&self, message: Value) {
        if !self.active() {
            self.close();
            return;
        }
        if message.get("type").and_then(Value::as_str) == Some("hello") {
            self.post(json!({"type":"hello"}));
            return;
        }
        let Some(request) = message
            .get("request")
            .and_then(Value::as_u64)
            .filter(|id| *id > 0 && *id <= 9_007_199_254_740_991)
        else {
            return;
        };
        if self.pending.borrow().contains(&request) {
            self.close();
            return;
        }
        let parse = || -> Result<(Area, Context, Operation), String> {
            if self.pending.borrow().len() >= MAX_PENDING {
                return Err("Too many pending storage requests.".into());
            }
            let area = serde_json::from_value(
                message
                    .get("area")
                    .cloned()
                    .ok_or("Missing storage area.")?,
            )
            .map_err(|_| "Invalid storage area.")?;
            let operation = serde_json::from_value(
                message
                    .get("operation")
                    .cloned()
                    .ok_or("Missing storage operation.")?,
            )
            .map_err(|_| "Invalid storage operation.")?;
            let context = match message.get("contentScript").and_then(Value::as_bool) {
                Some(true) => Context::ContentScript,
                Some(false) => Context::Trusted,
                None => return Err("Missing storage context.".into()),
            };
            Ok((area, context, operation))
        };
        let (area, context, operation) = match parse() {
            Ok(value) => value,
            Err(error) => {
                self.reply(request, Err(error));
                return;
            }
        };
        self.pending.borrow_mut().insert(request);
        let key = self.key;
        if let Err(error) = self.client.broker.execute(
            self.client.installation.clone(),
            area,
            context,
            operation,
            Box::new(move |result| {
                dispatch2::DispatchQueue::main().exec_async(move || {
                    let connection = PORTS.with(|ports| ports.borrow().get(&key).cloned());
                    if let Some(connection) = connection {
                        connection.pending.borrow_mut().remove(&request);
                        if connection.active() {
                            connection.reply(request, result);
                        }
                    }
                });
            }),
        ) {
            self.pending.borrow_mut().remove(&request);
            self.reply(request, Err(error));
        }
    }

    fn reply(&self, request: u64, result: Result<Value, String>) {
        self.post(match result {
            Ok(value) => json!({"request":request,"value":value}),
            Err(error) => json!({"request":request,"error":error}),
        });
    }

    fn post(&self, message: Value) {
        if self.closed.get() {
            return;
        }
        let object = json::to_object(&message);
        unsafe {
            self.port.sendMessage_completionHandler(Some(&object), None);
        }
    }

    fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        PORTS.with(|ports| ports.borrow_mut().remove(&self.key));
        self.pending.borrow_mut().clear();
        unsafe {
            self.port.disconnect();
        }
    }
}

pub(crate) fn close_extension(shared: &Shared, extension: &str) {
    let ports: Vec<_> = PORTS.with(|ports| {
        ports
            .borrow()
            .values()
            .filter(|connection| {
                connection.owner.as_ptr() == shared as *const _
                    && connection.client.installation.extension.as_str() == extension
            })
            .cloned()
            .collect()
    });
    for connection in ports {
        connection.close();
    }
}

pub fn notify(change: Changed) {
    dispatch2::DispatchQueue::main().exec_async(move || {
        let ports: Vec<_> = PORTS.with(|ports| ports.borrow().values().filter(|connection| connection.client.installation.extension == change.installation.extension && connection.client.installation.key == change.installation.key).cloned().collect());
        for connection in ports {
            if connection.active() { connection.post(json!({"type":"changed","area":change.area,"changes":change.changes,"contentScripts":change.content_scripts})); }
        }
    });
}
