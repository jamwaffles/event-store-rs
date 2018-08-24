//! Adapters for use with event store integrations
//!
//! A store will require a storage backend, cache backend and an event emitter instance for
//! integration with other event-driven domains. Use the adapters exposed by this module to suit
//! your application and architecture.

mod pg;
mod stub;

pub use self::pg::{PgCacheAdapter, PgQuery, PgStoreAdapter};
pub use self::stub::StubEmitterAdapter;

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use Aggregator;
use Events;
use StoreQuery;

/// Storage backend
pub trait StoreAdapter<E: Events, Q: StoreQuery> {
    /// Read a list of events matching a query
    fn aggregate<T, A>(
        &self,
        query_args: A,
        since: Option<(T, DateTime<Utc>)>,
    ) -> Result<T, String>
    where
        T: Aggregator<E, A, Q> + DeserializeOwned + Default,
        A: Clone;

    /// Save an event to the store
    fn save<S>(&self, event: &E, subject: Option<S>) -> Result<(), String>
    where
        S: Serialize;
}

/// Caching backend
pub trait CacheAdapter<K, V: Serialize + DeserializeOwned> {
    /// Insert an item into the cache
    fn insert(&self, key: &K, value: V);

    /// Retrieve an item from the cache
    fn get(&self, key: &K) -> Option<(V, DateTime<Utc>)>;
}

/// Closure called when an incoming event must be handled
pub type EventHandler<E> = fn(&E) -> ();

/// Event emitter interface
pub trait EmitterAdapter<E: Events> {
    /// Get all subscribed handlers
    fn get_subscriptions(&self) -> HashMap<String, EventHandler<E>>;

    /// Emit an event
    fn emit(&self, event: &E);

    /// Subscribe to an event
    fn subscribe<H>(&mut self, event_name: String, handler: EventHandler<E>);

    /// Stop listening for an event
    fn unsubscribe(&mut self, event_name: String);
}