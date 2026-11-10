mod protocol;
pub(super) use protocol::{accepts_command, allows_origin};

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
            let _ = view.evaluate_script(&script);
        }
    }
}

fn stylesheet(appearance: DeckAppearance, hide_advertisements: bool) -> String {
    let padding = if appearance.density == Density::Compact {
        "4px 8px"
    } else {
        "8px 10px"
    };
    let radius = if appearance.avatars == AvatarShape::Square {
        "4px"
    } else {
        "50%"
    };
    let mut css = format!(":root{{--bdeck-font-size:{}px;--bdeck-post-padding:{padding};--bdeck-avatar-radius:{radius};}}", appearance.font_size);
    css.push_str(include_str!("styles/columns.css"));
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
  const applyChat = () => {{
    const root = document.querySelector('[data-testid="xchatEmbedRoute"]')?.shadowRoot;
    if (!root || root.getElementById("bdeck-chat-style")) return;
    const style = document.createElement("style");
    style.id = "bdeck-chat-style";
    style.textContent = {chat_css};
    root.appendChild(style);
  }};
  const applyHomeComposer = () => {{
    const home = location.pathname === "/home";
    document.documentElement.toggleAttribute("data-bdeck-home", home);
    const primary = document.querySelector('[data-testid="primaryColumn"]');
    primary?.querySelectorAll(".bdeck-home-composer").forEach(node => node.classList.remove("bdeck-home-composer"));
    if (!home) return;
    const button = primary?.querySelector('[data-testid="tweetButtonInline"]');
    const editor = primary?.querySelector('[data-testid="tweetTextarea_0"]');
    if (!button || !editor) return;
    for (let node = button.parentElement; node && node !== primary; node = node.parentElement) {{
      if (node.querySelector('[data-testid="tweet"]')) return;
      if (node.contains(editor) && node.querySelector('[data-testid^="UserAvatar-Container"]')) {{
        node.classList.add("bdeck-home-composer");
        return;
      }}
    }}
  }};
  const applyPostSpacing = () => {{
    document.querySelectorAll('[data-testid="primaryColumn"] [data-testid="tweet"]').forEach(article => {{
      article.querySelectorAll(".bdeck-post-body, .bdeck-empty-context").forEach(node => node.classList.remove("bdeck-post-body", "bdeck-empty-context"));
      const name = article.querySelector('[data-testid="User-Name"]');
      const avatar = article.querySelector('[data-testid^="UserAvatar-Container"]');
      if (!name || !avatar) return;
      for (let row = name.parentElement; row && row !== article; row = row.parentElement) {{
        if (!row.contains(avatar)) continue;
        let body = name;
        while (body.parentElement && body.parentElement !== row) body = body.parentElement;
        body.classList.add("bdeck-post-body");
        const context = row.previousElementSibling;
        if (context && !context.textContent.trim() && !context.querySelector('img, svg, a, button, [role="button"]')) context.classList.add("bdeck-empty-context");
        return;
      }}
    }});
  }};
  const apply = () => {{
    const style = document.getElementById("bdeck-column-style") || document.createElement("style");
    style.id = "bdeck-column-style";
    style.textContent = {css};
    (document.head || document.documentElement).appendChild(style);
    applyChat();
    applyHomeComposer();
    applyPostSpacing();
    if (!style.hasAttribute("data-chat-layout")) {{
      style.setAttribute("data-chat-layout", "");
      new MutationObserver(() => {{ applyChat(); applyHomeComposer(); applyPostSpacing(); }}).observe(document.documentElement, {{ childList: true, subtree: true }});
    }}
  }};
  if (document.documentElement) apply();
  else document.addEventListener("DOMContentLoaded", apply, {{ once: true }});
}})();"#
    )
}
