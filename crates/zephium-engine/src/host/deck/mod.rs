mod protocol;
mod zoom;
pub(super) use protocol::{accepts_command, allows_origin};
pub(super) use zoom::{content_scale, sync as sync_zoom};

use zephium_core::deck::{
    appearance::{AvatarShape, Density, MediaSize, ScrollbarStyle},
    DeckAppearance,
};
use zephium_core::ports::engine::ComposerFocus;

pub(super) fn initialization_script(
    appearance: DeckAppearance,
    hide_advertisements: bool,
) -> Option<String> {
    Some(script(appearance, hide_advertisements))
}
pub(super) fn update_script(appearance: DeckAppearance, hide_advertisements: bool) -> String {
    script(appearance, hide_advertisements)
}

impl super::EngineHost {
    pub(crate) fn focus_composer(&self, item: zephium_core::ids::ItemId, context: ComposerFocus) {
        self.focus(item);
        if let Some(view) = self.views.get(&item) {
            let mention = match context {
                ComposerFocus::Preserve => "undefined".into(),
                ComposerFocus::Recipient(recipient) => {
                    serde_json::to_string(&recipient).expect("recipient is valid JSON")
                }
            };
            let script = format!(
                "(() => {{ const mention = {mention}; const composerOverlay = {}; {} }})();",
                include_str!("composer-overlay.js"),
                include_str!("composer-focus.js")
            );
            let _ = view.evaluate_script(&script);
        }
    }

    pub(crate) fn set_deck_appearance(
        &mut self,
        appearance: DeckAppearance,
        hide_advertisements: bool,
    ) {
        self.deck_appearance = appearance;
        self.hide_advertisements = hide_advertisements;
        let script = update_script(appearance, hide_advertisements);
        for view in self.views.values() {
            if let Some(url) = crate::platform::imp::current_url(view) {
                sync_zoom(view, appearance, &url);
            }
            let _ = view.evaluate_script(&script);
        }
    }
}

fn stylesheet(appearance: DeckAppearance, hide_advertisements: bool) -> String {
    let radius = if appearance.avatars == AvatarShape::Square {
        "4px"
    } else {
        "50%"
    };
    let mut css = format!(":root{{--bdeck-font-size:14px;--bdeck-avatar-radius:{radius};}}");
    css.push_str(include_str!("styles/columns.css"));
    if appearance.density == Density::Compact {
        css.push_str(include_str!("styles/posts-compact.css"));
    }
    css.push_str(include_str!("styles/avatars.css"));
    css.push_str(include_str!("styles/composer.css"));
    if hide_advertisements {
        css.push_str(include_str!("styles/advertisements.css"));
    }
    if appearance.hide_navigation {
        css.push_str(include_str!("styles/navigation.css"));
    }
    match appearance.media {
        MediaSize::Native => {}
        MediaSize::Compact => css.push_str(include_str!("styles/media-compact.css")),
        MediaSize::Hidden => css.push_str(include_str!("styles/media-hidden.css")),
    }
    match appearance.scrollbars {
        ScrollbarStyle::System => {}
        ScrollbarStyle::Thin => css.push_str(include_str!("styles/scrollbars-thin.css")),
        ScrollbarStyle::Hidden => css.push_str(include_str!("styles/scrollbars-hidden.css")),
    }
    css
}

fn script(appearance: DeckAppearance, hide_advertisements: bool) -> String {
    let css = serde_json::to_string(&stylesheet(appearance, hide_advertisements))
        .expect("stylesheet is valid JSON");
    let chat_css =
        serde_json::to_string(include_str!("styles/chat.css")).expect("stylesheet is valid JSON");
    format!(
        r#"(() => {{
  const host = location.hostname.toLowerCase();
  if (!(host === "x.com" || host.endsWith(".x.com") || host === "twitter.com" || host.endsWith(".twitter.com"))) return;
  const columnCss = {css};
  const chatCss = {chat_css};
  const createHomePreference = {};
  const createHomeSelection = {};
  {}
}})();"#,
        include_str!("home-preference.js"),
        include_str!("home-selection.js"),
        include_str!("layout.js")
    )
}
