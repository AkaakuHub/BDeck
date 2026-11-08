#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub(crate) struct AccountOption {
    pub id: String,
    pub title: String,
    pub screen_name: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub(crate) struct PanelView {
    pub composer: bool,
    pub id: Option<String>,
    pub title: String,
    pub index: u16,
    pub count: u16,
    pub account: String,
    pub accounts: Vec<AccountOption>,
    pub width: u16,
    pub row: u8,
    pub two_rows: bool,
    pub suspended: bool,
    pub failed: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum PanelAction {
    Create(ColumnDraft),
    Width(u16),
    Account(String),
    Row(u8),
    Previous,
    Next,
    Duplicate,
    Suspend,
    Remove,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ColumnKind {
    Home,
    Notifications,
    Messages,
    Explore,
    Search,
    List,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(deny_unknown_fields)]
pub(crate) struct ColumnDraft {
    pub kind: ColumnKind,
    pub account: String,
    pub text: String,
    pub width: u16,
    pub row: u8,
}

impl ColumnDraft {
    pub(super) fn valid(&self, view: &PanelView) -> bool {
        use zephium_core::deck::{ColumnTarget, MAX_COLUMN_WIDTH, MIN_COLUMN_WIDTH};
        let target = match self.kind {
            ColumnKind::Home => ColumnTarget::Home,
            ColumnKind::Notifications => ColumnTarget::Notifications,
            ColumnKind::Messages => ColumnTarget::Url {
                url: "https://x.com/messages".into(),
            },
            ColumnKind::Explore => ColumnTarget::Url {
                url: "https://x.com/explore".into(),
            },
            ColumnKind::Search => ColumnTarget::Search {
                query: self.text.trim().into(),
            },
            ColumnKind::List => ColumnTarget::List {
                url: self.text.trim().into(),
            },
        };
        !view.composer
            && view.id.is_none()
            && view
                .accounts
                .iter()
                .any(|account| account.id == self.account)
            && (MIN_COLUMN_WIDTH..=MAX_COLUMN_WIDTH).contains(&self.width)
            && self.row <= 1
            && target.valid()
    }
}
