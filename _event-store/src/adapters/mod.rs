//! Adapters for use with event store integrations
//!
//! A store will require a storage backend, cache backend and an event emitter instance for
//! integration with other event-driven domains. Use the adapters exposed by this module to suit
//! your application and architecture.

// TODO: No pub
pub mod amqp;
mod pg;
mod stub;

pub use self::amqp::AMQPEmitterAdapter;
pub use self::pg::{PgCacheAdapter, PgQuery, PgStoreAdapter};
pub use self::stub::StubEmitterAdapter;
use chrono::{DateTime, Utc};
use event_store_derive_internals::EventData;
use serde::{de::DeserializeOwned, Serialize};
use std::io;
use utils::BoxedFuture;
use Event;
use Events;
use StoreQuery;

/// Storage backend
pub trait StoreAdapter<Q: StoreQuery>: Send + Sync + Clone + 'static {
    /// Read a list of events matching a query

    fn read<'b, E>(&self, query: Q, since: Option<DateTime<Utc>>) -> Result<Vec<E>, String>
    where
        E: Events + Send + 'b,
        Q: 'b;
    /// Save an event to the store
    fn save<'b, ED>(&self, event: &'b Event<ED>) -> Result<(), String>
    where
        ED: EventData + Send + Sync + 'b;

    /// Returns the last event of the type ED
    fn last_event<'b, ED: EventData + Send + 'b>(&self) -> Result<Option<Event<ED>>, String>;
}

/// Result of a cache search
pub type CacheResult<T> = (T, DateTime<Utc>);

/// Caching backend
pub trait CacheAdapter {
    /// Insert an item into the cache
    fn set<'a, V>(&self, key: String, value: V) -> Result<(), String>
    where
        V: Serialize + Send + 'a;

    /// Retrieve an item from the cache
    fn get<'a, T>(&self, key: String) -> Result<Option<CacheResult<T>>, String>
    where
        T: DeserializeOwned + Send + 'a;
}

/// Closure called when an incoming event must be handled

/// Event emitter interface
pub trait EmitterAdapter: Send + Sync + Clone + 'static {
    /// Emit an event
    fn emit<'a, E: EventData + Sync>(&self, event: &Event<E>) -> Result<(), io::Error>;

    /// Subscribe to an event
    fn subscribe<'a, ED, H>(&self, handler: H) -> BoxedFuture<'a, (), ()>
    where
        ED: EventData + 'static,
        H: Fn(&Event<ED>) -> () + Send + Sync + 'static;
}