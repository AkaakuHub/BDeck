//! Persistent entities use ULIDs (sortable, globally unique, sync-safe).
//! Runtime-only entities (windows) use local u64.

use std::fmt;

use ulid::Ulid;

macro_rules! ulid_id {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[cfg_attr(feature = "ipc-types", derive(specta::Type))]
        #[cfg_attr(feature = "ipc-types", specta(type = String))]
        pub struct $name(Ulid);

        impl $name {
            /// Minted at the shell edge; aggregates receive ids, never create them.
            pub fn generate() -> Self {
                Self(Ulid::new())
            }

            pub fn parse(s: &str) -> Option<Self> {
                Ulid::from_string(s).ok().map(Self)
            }

            pub fn bytes(self) -> [u8; 16] {
                self.0 .0.to_be_bytes()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Self::parse(&value)
                    .ok_or_else(|| serde::de::Error::custom(concat!("invalid ", stringify!($name))))
            }
        }

        impl From<u128> for $name {
            fn from(n: u128) -> Self {
                Self(Ulid(n))
            }
        }
    };
}

ulid_id!(DownloadId);
ulid_id!(ClosedSessionId);
ulid_id!(ItemId);
ulid_id!(SpaceId);
ulid_id!(ProfileId);
ulid_id!(ExtensionInstallId);
ulid_id!(PagePermissionGrantId);
ulid_id!(DeckId);
ulid_id!(DeckColumnId);
ulid_id!(DeckAccountId);

pub type WindowId = u64;

impl ExtensionInstallId {
    pub fn generate_after(high_water: Option<Self>) -> Option<Self> {
        Self::candidate_after(Self::generate(), high_water)
    }

    fn candidate_after(candidate: Self, high_water: Option<Self>) -> Option<Self> {
        let Some(high_water) = high_water else {
            return Some(candidate);
        };
        if candidate > high_water {
            return Some(candidate);
        }
        high_water.0 .0.checked_add(1).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
