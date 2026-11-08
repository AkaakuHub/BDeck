use super::super::{dispatch, EngineHost};
use serde::Deserialize;
use serde_json::{json, Value};
use zephium_core::extensions::ExtensionRuntimeInstance;
use zephium_webext::storage::{
    broker::{Changed, Client},
    Area, Context, Operation,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    kind: Kind,
    request: u64,
    area: Area,
    operation: Operation,
    #[serde(rename = "contentScript")]
    content_script: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Storage,
}

pub(super) fn execute(
    client: &Client,
    runtime: ExtensionRuntimeInstance,
    value: Value,
    main: crate::MainThreadDispatch,
) {
    let Ok(request) = serde_json::from_value::<Request>(value) else {
        return;
    };
    let Kind::Storage = request.kind;
    if request.request == 0 || request.request > 9_007_199_254_740_991 {
        return;
    }
    let id = request.request;
    let completion = main.clone();
    let result = client.broker.execute(
        client.installation.clone(),
        request.area,
        if request.content_script {
            Context::ContentScript
        } else {
            Context::Trusted
        },
        request.operation,
        Box::new(move |result| reply(completion, runtime, id, result)),
    );
    if let Err(error) = result {
        reply(main, runtime, id, Err(error));
    }
}

fn reply(
    main: crate::MainThreadDispatch,
    runtime: ExtensionRuntimeInstance,
    id: u64,
    result: Result<Value, String>,
) {
    let value = match result {
        Ok(value) => json!({"kind":"storage", "request":id, "value":value}),
        Err(error) => json!({"kind":"storage", "request":id, "error":error}),
    };
    let _ = main(Box::new(move || {
        dispatch::best_effort_with(move |host| {
            host.post_windows_extension_storage(runtime, &value)
        });
    }));
}

impl EngineHost {
    fn post_windows_extension_storage(&self, runtime: ExtensionRuntimeInstance, value: &Value) {
        let Some(install) = self
            .windows_extensions
            .installs
            .get(&(runtime.profile(), runtime.install_id()))
            .filter(|install| install.runtime == runtime && install.bridge.alive.get())
        else {
            return;
        };
        let encoded: Vec<u16> = value.to_string().encode_utf16().chain(Some(0)).collect();
        use wry::WebViewExtWindows;
        // The exact extension generation owns this hidden controller on the UI apartment.
        if let Err(error) = unsafe {
            install
                .bridge
                .view
                .webview()
                .PostWebMessageAsJson(windows::core::PCWSTR(encoded.as_ptr()))
        } {
            eprintln!("extensions: storage response delivery failed: {error}");
        }
    }

    pub(crate) fn notify_windows_extension_storage(&self, change: Changed) {
        let value = json!({"kind":"storage", "type":"changed", "area":change.area,
            "changes":change.changes, "contentScripts":change.content_scripts});
        for install in self.windows_extensions.installs.values().filter(|install| {
            install.id == change.installation.extension.as_str()
                && install.runtime.install_id().bytes() == change.installation.key
        }) {
            self.post_windows_extension_storage(install.runtime, &value);
        }
    }
}
