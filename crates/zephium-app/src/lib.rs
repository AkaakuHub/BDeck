//! The actor shell around the pure core. One thread owns all state; commands
//! enter through a queue (UI intents and engine events alike), effects leave
//! through ports, projections go to the UI.

mod diagnostics;

macro_rules! diagnostic {
    ($($argument:tt)*) => {{
        crate::diagnostics::write(format_args!($($argument)*));
    }};
}

pub(crate) use diagnostic;

mod actor;
mod api;
mod reply;
mod shell;
mod store_reads;

pub use actor::{
    spawn, spawn_suspended, CallbackHandle, Handle, ShutdownRequest, SpawnError, SpawnFailure,
};
pub use api::{
    ChromePresentation, ChromePresentationCallback, ChromePresentationDispatch, Command, EmitFn,
    PagePermissionPromptDecision, PageRequestDecision, PresentationChrome, SharedChrome,
    SharedEngine, SharedStore, ShellTerminalFailure, ShellTerminalFailureCallback, ShutdownOutcome,
    TabMetadata,
};
pub use shell::{Shell, WebExtensionStatus, WebExtensionTarget};

pub use api::{
    FaviconPageSource, FaviconProbe, FaviconProber, FaviconProberAttachment, SpaceCommand,
    TabAction, TabCompletion,
};
#[doc(hidden)]
pub use store_reads::StoreReadResult;
