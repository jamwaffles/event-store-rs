use crate::adapters::{AmqpEmitterAdapter, PgCacheAdapter, PgQuery, PgStoreAdapter};
use crate::aggregator::Aggregator;
use crate::event::Event;
use crate::store_query::StoreQuery;
use chrono::prelude::*;
use event_store_derive_internals::EventData;
use event_store_derive_internals::Events;
use log::{debug, trace};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt::Debug;
use std::io;

#[derive(Clone)]
pub struct Store {
    store: PgStoreAdapter,
    cache: PgCacheAdapter,
    emitter: AmqpEmitterAdapter,
}

impl Store {
    pub fn new(store: PgStoreAdapter, cache: PgCacheAdapter, emitter: AmqpEmitterAdapter) -> Self {
        Self {
            store,
            cache,
            emitter,
        }
    }

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

        let cache_result = await!(self.cache.read(cache_key))?;

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

        Ok(events.iter().fold(initial_state, T::apply_event))
    }

    pub async fn save<'a, ED>(&'a self, event: &'a Event<ED>) -> Result<(), io::Error>
    where
        ED: EventData + Debug + Send + Sync,
    {
        debug!("Save and emit event {:?}", event);

        await!(self.save_no_emit(&event))?;

        await!(self.emitter.emit(&event))
    }

    pub async fn save_no_emit<'a, ED>(&'a self, event: &'a Event<ED>) -> Result<(), io::Error>
    where
        ED: EventData + Debug + Send + Sync,
    {
        debug!("Save (no emit) event {:?}", event);

        await!(self.store.save(&event))
    }

    pub async fn last_event<ED>(&self) -> Result<Option<Event<ED>>, io::Error>
    where
        ED: EventData,
    {
        self.store.last_event::<ED>()
    }

    pub async fn emit<'a, ED>(&'a self, event: &'a Event<ED>) -> Result<(), io::Error>
    where
        ED: EventData,
    {
        await!(self.emitter.emit(event))
    }

    pub(crate) async fn emit_value<'a, V>(
        &'a self,
        event_type: &'a str,
        event_namespace: &'a str,
        data: &'a V,
    ) -> Result<(), io::Error>
    where
        V: Serialize,
    {
        await!(self.emitter.emit_value(event_type, event_namespace, data))
    }

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
