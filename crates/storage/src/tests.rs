use crate::InMemoryStorage;
use trading_domain::StoragePort;

#[tokio::test]
async fn claims_each_event_once_across_engine_instances() {
    let storage = InMemoryStorage::new();
    assert!(storage.claim_event("source-123").await.expect("claim"));
    assert!(!storage.claim_event("source-123").await.expect("duplicate"));
}
