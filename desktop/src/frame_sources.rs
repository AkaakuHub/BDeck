//! Source anchors for privileged frame markup and bootstrap contract tests.
//! A frontend move changes its path here; assertions stay with the native contract.

pub const SRC_STYLES_TOKENS_CSS: &str = include_str!("../../frame/src/styles/global.css");
pub const SRC_BOOTSTRAP_CSS: &str = include_str!("../../frame/src/styles/global.css");
pub const INDEX_HTML: &str = include_str!("../../frame/browser.html");
pub const SRC_APP_APP_SVELTE: &str = include_str!("../../frame/src/app/browser/BrowserApp.svelte");
pub const SRC_MAIN_TS: &str = include_str!("../../frame/src/entries/browser.ts");
pub const SRC_APP_SHELL_SVELTE: &str = include_str!("../../frame/src/app/browser/Shell.svelte");
