use crate::deck::{appearance::HIDE_ADVERTISEMENTS_SETTING, DeckAppearance};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub appearance: DeckAppearance,
    pub hide_advertisements: bool,
    pub theme: String,
    pub automatic_updates: bool,
}

impl Preferences {
    pub fn read(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        Self {
            appearance: read("deck.appearance")
                .and_then(|value| serde_json::from_str::<DeckAppearance>(&value).ok())
                .filter(DeckAppearance::valid)
                .unwrap_or_default(),
            hide_advertisements: read(HIDE_ADVERTISEMENTS_SETTING).as_deref() == Some("true"),
            theme: read("appearance")
                .filter(|value| crate::preferences::value_allowed("appearance", value))
                .unwrap_or_else(|| "system".into()),
            automatic_updates: read("updates.auto-check").as_deref() != Some("false"),
        }
    }

    pub fn valid(&self) -> bool {
        self.appearance.valid() && crate::preferences::value_allowed("appearance", &self.theme)
    }

    pub fn values(&self) -> [(String, String); 4] {
        [
            (
                "deck.appearance".into(),
                serde_json::to_string(&self.appearance).expect("appearance serializes"),
            ),
            (
                HIDE_ADVERTISEMENTS_SETTING.into(),
                self.hide_advertisements.to_string(),
            ),
            ("appearance".into(), self.theme.clone()),
            (
                "updates.auto-check".into(),
                self.automatic_updates.to_string(),
            ),
        ]
    }
}
