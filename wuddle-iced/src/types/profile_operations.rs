/// Immutable identity captured when profile-scoped asynchronous work begins.
///
/// The generation distinguishes an old visit to a profile from the current
/// visit, including the A -> B -> A case where the profile ID alone matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileOperationScope {
    pub profile_id: String,
    pub generation: u64,
}

impl ProfileOperationScope {
    pub fn new(profile_id: impl Into<String>, generation: u64) -> Self {
        Self {
            profile_id: profile_id.into(),
            generation,
        }
    }

    pub fn matches(&self, profile_id: &str, generation: u64) -> bool {
        self.profile_id == profile_id && self.generation == generation
    }
}

/// A task result tied to the exact profile context that started it.
#[derive(Debug, Clone)]
pub struct ProfileScoped<T> {
    pub scope: ProfileOperationScope,
    pub value: T,
}

impl<T> ProfileScoped<T> {
    pub fn new(scope: ProfileOperationScope, value: T) -> Self {
        Self { scope, value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_rejects_another_profile() {
        let scope = ProfileOperationScope::new("profile-a", 4);
        assert!(!scope.matches("profile-b", 4));
    }

    #[test]
    fn scope_rejects_an_older_visit_to_the_same_profile() {
        let scope = ProfileOperationScope::new("profile-a", 4);
        assert!(!scope.matches("profile-a", 5));
    }

    #[test]
    fn profile_a_to_b_to_a_does_not_revalidate_the_old_result() {
        let old_a_result = ProfileOperationScope::new("profile-a", 10);
        let profile_b_generation = 11;
        let new_a_generation = 12;

        assert!(!old_a_result.matches("profile-b", profile_b_generation));
        assert!(!old_a_result.matches("profile-a", new_a_generation));
        assert!(ProfileOperationScope::new("profile-a", new_a_generation)
            .matches("profile-a", new_a_generation));
    }
}
