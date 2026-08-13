use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Returned when an opaque store identifier cannot be represented safely as one path segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdParseError {
    value: String,
}

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid identifier {:?}: use 4 to 64 ASCII alphanumeric or '-' characters",
            self.value
        )
    }
}

impl std::error::Error for IdParseError {}

fn validate(value: &str) -> Result<(), IdParseError> {
    let valid_length = (4..=64).contains(&value.len());
    let valid_chars = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if valid_length && valid_chars {
        Ok(())
    } else {
        Err(IdParseError {
            value: value.to_owned(),
        })
    }
}

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            // An opaque id has no meaningful `Default`; `new()` deliberately mints a
            // fresh UUIDv7, so a blanket `Default` impl would be a misuse hazard.
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self(uuid::Uuid::now_v7().to_string())
            }

            pub fn parse(value: impl AsRef<str>) -> Result<Self, IdParseError> {
                let value = value.as_ref();
                validate(value)?;
                Ok(Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

typed_id!(ProjectId);
typed_id!(ProjectSourceId);
typed_id!(WorkItemId);
typed_id!(CommitmentTransitionId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_accepts_legacy_and_rejects_paths() {
        assert_eq!(
            ProjectId::parse("b8a9e19ef3c91245").unwrap().as_str(),
            "b8a9e19ef3c91245"
        );
        assert_eq!(WorkItemId::parse("a1b2").unwrap().as_str(), "a1b2");
        assert!(ProjectId::parse("../projects/other").is_err());
        assert!(ProjectId::parse("").is_err());
    }

    #[test]
    fn generated_project_id_is_uuid_v7() {
        let id = ProjectId::new();
        assert_eq!(
            uuid::Uuid::parse_str(id.as_str())
                .unwrap()
                .get_version_num(),
            7
        );
    }

    #[test]
    fn typed_ids_serde_round_trip_as_strings() {
        let project = ProjectId::parse("project-2026").unwrap();
        let source = ProjectSourceId::parse("source-2026").unwrap();
        let work_item = WorkItemId::parse("a1b2").unwrap();
        let transition = CommitmentTransitionId::parse("trans-2026").unwrap();

        assert_eq!(serde_json::to_string(&project).unwrap(), "\"project-2026\"");
        assert_eq!(
            serde_json::from_str::<ProjectSourceId>("\"source-2026\"").unwrap(),
            source
        );
        assert_eq!(
            serde_json::from_str::<WorkItemId>("\"a1b2\"").unwrap(),
            work_item
        );
        assert_eq!(
            serde_json::from_str::<CommitmentTransitionId>("\"trans-2026\"").unwrap(),
            transition
        );
    }

    #[test]
    fn typed_ids_reject_invalid_serde_values() {
        assert!(serde_json::from_str::<ProjectId>("\"../other-project\"").is_err());
    }

    #[test]
    fn typed_ids_reject_nonportable_characters_and_invalid_lengths() {
        for raw in [
            "abc",
            "name/child",
            "name\\child",
            "name.value",
            "name_value",
        ] {
            assert!(ProjectId::parse(raw).is_err(), "{raw} must be rejected");
        }
        assert!(ProjectId::parse("a".repeat(65)).is_err());
    }
}
