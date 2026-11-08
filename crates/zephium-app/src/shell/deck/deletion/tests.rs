use super::*;
use session::{PersistedItem, PersistedKind, PersistedProfile, PersistedSpace, SessionState};

fn fixture() -> (SessionState, HashMap<ItemId, ProfileId>) {
    let root = ProfileId::from(1);
    let other = ProfileId::from(2);
    let space = SpaceId::from(1);
    let first = ItemId::from(1);
    let second = ItemId::from(2);
    let compose = ItemId::from(3);
    let state = SessionState {
        profiles: vec![
            PersistedProfile {
                id: root,
                name: "main".into(),
                kind: ProfileKind::Default,
            },
            PersistedProfile {
                id: other,
                name: "Account".into(),
                kind: ProfileKind::Named,
            },
        ],
        spaces: vec![
            PersistedSpace {
                id: space,
                profile: root,
                name: "Space".into(),
                look: Default::default(),
            },
            PersistedSpace {
                id: SpaceId::from(2),
                profile: other,
                name: "Space".into(),
                look: Default::default(),
            },
        ],
        items: [first, second, compose]
            .map(|id| PersistedItem {
                id,
                parent: None,
                placement: Placement::Space {
                    space,
                    section: zephium_core::item::SpaceSection::Today,
                },
                kind: PersistedKind::Tab {
                    url: "https://x.com/home".into(),
                    title: "X".into(),
                    zoom: 1.0,
                    last_active_ms: None,
                },
            })
            .into(),
        active_space: Some(space),
        active_item: Some(first),
        splits: Some(Pane::Branch {
            axis: Axis::Row,
            ratio: 0.5,
            a: Box::new(Pane::Leaf(first)),
            b: Box::new(Pane::Leaf(second)),
        }),
        recently_closed: Vec::new(),
    };
    assert_eq!(session::canonicalize(state.clone()), state);
    (
        state,
        [(first, root), (second, other), (compose, other)].into(),
    )
}

#[test]
fn removing_an_account_uses_the_view_owner_instead_of_its_layout_space() {
    let (state, owners) = fixture();
    let filtered = filter_session(state, ProfileId::from(2), &owners).unwrap();
    assert_eq!(
        filtered
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![ItemId::from(1)]
    );
    assert_eq!(filtered.profiles.len(), 1);
    assert_eq!(filtered.active_item, Some(ItemId::from(1)));
    assert_eq!(filtered.splits, Some(Pane::Leaf(ItemId::from(1))));
}

#[test]
fn removing_the_default_account_preserves_other_accounts_and_their_cached_composer() {
    let (state, owners) = fixture();
    let filtered = filter_session(state, ProfileId::from(1), &owners).unwrap();
    assert_eq!(
        filtered
            .profiles
            .iter()
            .map(|profile| profile.id)
            .collect::<Vec<_>>(),
        vec![ProfileId::from(2)]
    );
    assert_eq!(
        filtered
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![ItemId::from(2), ItemId::from(3)]
    );
    assert_eq!(filtered.active_space, Some(SpaceId::from(2)));
    for item in filtered.items {
        assert_eq!(
            item.placement,
            Placement::Space {
                space: SpaceId::from(2),
                section: zephium_core::item::SpaceSection::Today
            }
        );
    }
    assert_eq!(filtered.splits, Some(Pane::Leaf(ItemId::from(2))));
}

#[test]
fn the_final_account_has_no_persisted_login_or_item_references() {
    let (state, owners) = fixture();
    let first = filter_session(state, ProfileId::from(1), &owners).unwrap();
    let empty = filter_session(first, ProfileId::from(2), &owners).unwrap();
    assert_eq!(empty, SessionState::default());
}
