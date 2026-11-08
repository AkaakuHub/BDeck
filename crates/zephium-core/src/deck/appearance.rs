pub const HIDE_ADVERTISEMENTS_SETTING: &str = "deck.hide-advertisements";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AvatarShape {
    #[default]
    Circle,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MediaSize {
    #[default]
    Native,
    Compact,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ScrollbarStyle {
    System,
    #[default]
    Thin,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DeckAppearance {
    pub font_size: u8,
    pub density: Density,
    pub avatars: AvatarShape,
    pub media: MediaSize,
    pub scrollbars: ScrollbarStyle,
    pub hide_navigation: bool,
}

impl Default for DeckAppearance {
    fn default() -> Self {
        Self {
            font_size: 14,
            density: Density::Comfortable,
            avatars: AvatarShape::Circle,
            media: MediaSize::Native,
            scrollbars: ScrollbarStyle::Thin,
            hide_navigation: true,
        }
    }
}

impl DeckAppearance {
    pub fn valid(&self) -> bool {
        (12..=18).contains(&self.font_size)
    }
}
