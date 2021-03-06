use crate::adapters::{
    AmqpEmitterAdapter, PgCacheAdapter, PgQuery, PgStoreAdapter, SaveResult, SaveStatus,
};
use crate::aggregator::Aggregator;
use crate::event::Event;
use crate::store_query::StoreQuery;
use chrono::prelude::*;
use event_store_derive_internals::EventData;
use event_store_derive_internals::Events;
use log::{debug, trace};
use serde_json::Value as JsonValue;
use std::fmt::Debug;
use std::io;

/// Event store that does not support subscriptions. Passed to [`crate::event_handler::EventHandler`] implementations.
#[derive(Clone)]
pub struct Store {
    pub(crate) store: PgStoreAdapter,
    cache: PgCacheAdapter,
    emitter: AmqpEmitterAdapter,
}

impl Store {
    /// Create a new non-subscribable store
    pub fn new(store: PgStoreAdapter, cache: PgCacheAdapter, emitter: AmqpEmitterAdapter) -> Self {
        Self {
            store,
            cache,
            emitter,
        }
    }

    /// Read events from the backing store, producing a reduced result
    pub async fn aggregate<'a, T, QA, E>(&'a self, query_args: &'a QA) -> Result<T, io::Error>
    where
        E: Events,
        T: Aggregator<E, QA, PgQuery>,
        QA: Clone + Debug + 'a,
    {
        debug!("Aggregate with arguments {:?}", query_args);

        let store_query = T::query(query_args.clone());
        let cache_key = store_query.unique_id();
        let debug_cache_key = cache_key.clone();

        let cache_result = await!(self.cache.read(&cache_key))?;

        trace!(
            "Aggregate cache key {} result {:?}",
            debug_cache_key,
            cache_result
        );

        let (initial_state, since) = cache_result
            .map(|res| (res.0, Some(res.1)))
            .unwrap_or_else(|| (T::default(), None));

        trace!(
            "Aggregate initial state {:?}, since {:?}",
            initial_state,
            since
        );

        let events = await!(self.store.read(&store_query, since))?;

        trace!("Read {} events to aggregate", events.len());

        let result = events.iter().fold(initial_state, T::apply_event);

        await!(self.cache.save(&cache_key, &result))?;

        Ok(result)
    }

    /// Save an event and emit it to other subscribers
    pub async fn save<'a, ED>(&'a self, event: &'a Event<ED>) -> SaveResult
    where
        ED: EventData + Debug,
    {
        debug!("Save and emit event {:?}", event);

        self.store.save(&event)?;

        await!(self.emitter.emit(&event)).map(|_| SaveStatus::Ok)
    }

    /// Emit an event to subscribers
    pub async fn emit<'a, ED>(&'a self, event: &'a Event<ED>) -> Result<(), io::Error>
    where
        ED: EventData,
    {
        await!(self.emitter.emit(event))
    }

    /// Read all events since a given time
    pub async fn read_events_since<'a>(
        &'a self,
        event_namespace: &'a str,
        event_type: &'a str,
        since: DateTime<Utc>,
    ) -> Result<Vec<JsonValue>, io::Error> {
        await!(self
            .store
            .read_events_since(event_namespace, event_type, since))
    }
}
