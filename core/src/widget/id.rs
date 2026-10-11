use std::borrow;
use std::sync::atomic::{self, AtomicUsize};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// The identifier of a generic widget.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(Internal);

impl Id {
    /// Creates a new [`Id`] from a static `str`.
    pub const fn new(id: &'static str) -> Self {
        Self(Internal::Custom(borrow::Cow::Borrowed(id)))
    }

    /// Creates a unique [`Id`].
    ///
    /// This function produces a different [`Id`] every time it is called.
    pub fn unique() -> Self {
        let id = NEXT_ID.fetch_add(1, atomic::Ordering::Relaxed);

        Self(Internal::Unique(id))
    }

    /// Groups several widget identifiers under one identifier.
    pub fn grouped(ids: impl IntoIterator<Item = Id>) -> Self {
        Self(Internal::Set(ids.into_iter().collect()))
    }

    /// Returns the identifiers in this group, if this is a grouped identifier.
    pub fn grouped_ids(&self) -> Option<&[Id]> {
        match &self.0 {
            Internal::Set(ids) => Some(ids),
            Internal::Unique(_) | Internal::Custom(_) => None,
        }
    }

    /// Returns a stable numeric identifier suitable for native drag-and-drop APIs.
    pub fn to_u128(&self) -> u128 {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        u128::from(hasher.finish())
    }
}

impl From<&'static str> for Id {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Id {
    fn from(value: String) -> Self {
        Self(Internal::Custom(borrow::Cow::Owned(value)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Internal {
    Unique(usize),
    Custom(borrow::Cow<'static, str>),
    Set(Vec<Id>),
}

#[cfg(feature = "a11y")]
impl From<Id> for iced_accessibility::A11yId {
    fn from(id: Id) -> Self {
        use iced_accessibility::id::{Id as AccessibilityId, Internal as AccessibilityInternal};

        fn convert(id: Id) -> AccessibilityId {
            match id.0 {
                // Keep core widget IDs separate from IDs allocated by the
                // accessibility crate for custom IDs.
                Internal::Unique(id) => AccessibilityId::from((1_u64 << 32) + id as u64),
                Internal::Custom(name) => AccessibilityId::new(name),
                Internal::Set(ids) => AccessibilityId(AccessibilityInternal::Set(
                    ids.into_iter().map(|id| convert(id).0).collect(),
                )),
            }
        }

        Self::from(convert(id))
    }
}

#[cfg(test)]
mod tests {
    use super::Id;

    #[test]
    fn unique_generates_different_ids() {
        let a = Id::unique();
        let b = Id::unique();

        assert_ne!(a, b);
    }

    #[cfg(feature = "a11y")]
    #[test]
    fn accessibility_ids_are_stable_and_namespaced() {
        use iced_accessibility::{A11yId, IdEq};

        let custom = Id::new("core-widget-custom-a11y-id");
        let first: A11yId = custom.clone().into();
        let second: A11yId = custom.into();
        let first_node = iced_accessibility::accesskit::NodeId::from(first.clone());
        let second_node = iced_accessibility::accesskit::NodeId::from(second);

        assert_eq!(first_node, second_node);
        assert!(IdEq::eq(&first, &A11yId::from(first_node)));

        let unique_node = iced_accessibility::accesskit::NodeId::from(A11yId::from(Id::unique()));
        assert_ne!(first_node, unique_node);
        assert_eq!(u64::from(unique_node.0) >> 32, 1);
    }
}
