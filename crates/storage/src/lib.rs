//! Storage adapters. PostgreSQL is durable; memory storage supports tests.

mod memory;
mod postgres;

pub use memory::InMemoryStorage;
pub use postgres::PostgresStorage;

#[cfg(test)]
mod tests;
