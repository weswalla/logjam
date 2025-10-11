/// Base DDD abstractions for the domain layer
use std::fmt::Debug;

/// Trait for value objects - immutable objects defined by their attributes
/// Value objects are equal if all their attributes are equal
pub trait ValueObject: Clone + PartialEq + Eq + Debug {}

/// Trait for entities - objects with identity that can change over time
/// Entities are equal if their IDs are equal, regardless of other attributes
pub trait Entity: Debug {
    type Id: ValueObject;

    fn id(&self) -> &Self::Id;
}

/// Trait for aggregate roots - entities that are the entry point to an aggregate
/// Aggregates ensure consistency boundaries and encapsulate business rules
pub trait AggregateRoot: Entity {
    /// Apply domain events and update the aggregate state
    fn apply_event(&mut self, event: &dyn DomainEvent);
}

/// Trait for domain events - things that have happened in the domain
pub trait DomainEvent: Debug + Clone {
    /// The name/type of the event
    fn event_type(&self) -> &'static str;

    /// When the event occurred (timestamp could be added in future)
    fn aggregate_id(&self) -> String;
}

/// Result type for domain operations
pub type DomainResult<T> = Result<T, DomainError>;

/// Domain-specific errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Invalid value provided
    InvalidValue(String),
    /// Entity not found
    NotFound(String),
    /// Business rule violation
    BusinessRuleViolation(String),
    /// Invalid operation
    InvalidOperation(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
            DomainError::NotFound(msg) => write!(f, "Not found: {}", msg),
            DomainError::BusinessRuleViolation(msg) => write!(f, "Business rule violation: {}", msg),
            DomainError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
        }
    }
}

impl std::error::Error for DomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestId(String);
    impl ValueObject for TestId {}

    #[derive(Debug)]
    struct TestEntity {
        id: TestId,
        value: String,
    }

    impl Entity for TestEntity {
        type Id = TestId;

        fn id(&self) -> &Self::Id {
            &self.id
        }
    }

    #[test]
    fn test_entity_has_identity() {
        let entity1 = TestEntity {
            id: TestId("test-1".to_string()),
            value: "original".to_string(),
        };

        let entity2 = TestEntity {
            id: TestId("test-1".to_string()),
            value: "modified".to_string(),
        };

        // Entities with same ID should be considered the same entity
        assert_eq!(entity1.id(), entity2.id());
    }

    #[test]
    fn test_domain_error_display() {
        let error = DomainError::InvalidValue("test".to_string());
        assert_eq!(error.to_string(), "Invalid value: test");
    }
}
