//! Event store for working with event-store-driven applications
#![deny(missing_docs)]

extern crate fallible_iterator;
#[macro_use]
extern crate serde_derive;
extern crate chrono;
#[macro_use]
extern crate event_store_derive;
extern crate event_store_derive_internals;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate sha2;
extern crate uuid;
#[macro_use]
extern crate log;
extern crate bb8;
extern crate bb8_postgres;
extern crate futures;
extern crate futures_state_stream;
extern crate lapin_futures as lapin;
extern crate tokio;

pub mod adapters;
mod aggregator;
mod event;
mod event_context;
pub mod prelude;
mod store;
mod store_query;
pub mod testhelpers;
mod utils;

use adapters::{CacheAdapter, CacheResult, EmitterAdapter, StoreAdapter};
use aggregator::Aggregator;
use chrono::prelude::*;
use event::Event;
use event_context::EventContext;
use event_store_derive_internals::{EventData, Events};

use futures::future::{ok as FutOk, Future};
use store::Store;

use serde::Serialize;
use store_query::StoreQuery;
use utils::BoxedFuture;
use uuid::Uuid;

/// Main event store
pub struct EventStore<S, C, EM> {
    store: S,
    cache: C,
    emitter: EM,
}

#[derive(Debug, Clone, EventData)]
#[event_store(namespace = "event_store")]
struct EventReplayRequested {
    requested_event_type: String,
    requested_event_namespace: String,
    since: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct DummyEvent {}

type OptionalCache<T> = Option<CacheResult<T>>;

impl<'a, SQ, S, C, EM> Store<'a, SQ, S, C, EM> for EventStore<S, C, EM>
where
    S: StoreAdapter + Send + Sync + Clone + 'static,
    C: CacheAdapter + Send + Sync + Clone + 'static,
    EM: EmitterAdapter + Send + Sync + Clone + 'static,
{
    /// Create a new event store
    fn new(store: S, cache: C, emitter: EM) -> Self {
        Self {
            store,
            cache,
            emitter,
        }
    }

    /// Query the backing store and return an entity `T`, reduced from queried events
    fn aggregate<'b, E, A, Q, T>(&self, query_args: A) -> BoxedFuture<'b, Option<T>, String>
    where
        E: Events,
        A: Serialize,
        Q: StoreQuery<'b, A>,
        T: Aggregator<'b, E, A, Q>,
    {
        let _q = T::query(query_args);
        let _result: BoxedFuture<OptionalCache<T>, String> = self.cache.get(String::from(""));
        Box::new(FutOk(None))
        // Box::new(FutResult(
        //     self.store
        //         .aggregate(query_args, initial_state.clone())
        //         .map(|agg| {
        //             if let Some((last_cache, _)) = initial_state {
        //                 // Only update cache if aggregation result has changed
        //                 if agg != last_cache {
        //                     self.cache.insert(&q, agg.clone());
        //                 }
        //             } else {
        //                 // If there is no existing cache item, insert one
        //                 self.cache.insert(&q, agg.clone());
        //             }

        //             agg
        //         }),
        // ))
    }

    /// Save an event to the store with optional context
    fn save<ED: EventData + Send + Sync + 'static>(
        &self,
        event: Event<ED>,
    ) -> BoxedFuture<(), String> {
        let tasks = self.store.save(event.clone()).and_then(move |_| {
            Box::new(
                self.emitter
                    .emit(&event)
                    .map_err(|_| "It was not possible to emit the event".into()),
            )
        });

        Box::new(tasks)
    }

    fn subscribe<ED, H>(&self, handler: H) -> BoxedFuture<(), String>
    where
        ED: EventData + Send + 'static,
        H: Fn(&Event<ED>) -> () + Send + Sync + 'static,
    {
        let handler_store = self.store.clone();
        Box::new(
            self.emitter
                .subscribe(move |event: &Event<ED>| {
                    let _ = handler_store.save(event.clone()).map(|_| {
                        handler(event);
                    });
                    /**/
                }).and_then(move |_| {
                    self.store
                        .last_event::<ED>()
                        .map(|o_event| {
                            o_event
                                .map(|event| event.context.time)
                                .unwrap_or_else(|| Utc::now())
                        }).or_else(|_| FutOk(Utc::now()))
                }).and_then(move |since| {
                    let data = EventReplayRequested {
                        requested_event_type: ED::event_type().into(),
                        requested_event_namespace: ED::event_namespace().into(),
                        since,
                    };
                    let id = Uuid::new_v4();
                    let context = EventContext {
                        action: None,
                        subject: None,
                        time: Utc::now(),
                    };
                    let event = Event { data, id, context };
                    self.emitter.emit(&event)
                }).map_err(|_| "It was not possible to subscribe".into()),
        )
    }
}
