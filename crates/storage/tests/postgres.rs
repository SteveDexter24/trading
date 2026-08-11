use trading_domain::StoragePort;
use trading_storage::PostgresStorage;
use uuid::Uuid;

#[tokio::test]
async fn migrations_enforce_event_idempotency_when_postgres_is_available() {
    let Ok(database_url) = std::env::var("TEST_DATABASE_URL") else {
        return;
    };
    let storage = PostgresStorage::connect(&database_url)
        .await
        .expect("connect to test PostgreSQL");
    storage.migrate().await.expect("apply migrations");
    let event_id = format!("integration-event-{}", Uuid::new_v4());

    assert!(storage.claim_event(&event_id).await.expect("first claim"));
    assert!(!storage
        .claim_event(&event_id)
        .await
        .expect("duplicate claim"));
}
