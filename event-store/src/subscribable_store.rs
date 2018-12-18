use crate::aggregator::Aggregator;
use crate::amqp::*;
use crate::event::Event;
use crate::event_handler::EventHandler;
use crate::event_replay::EventReplayRequested;
use crate::pg::*;
use crate::store::Store;
use chrono::naive::NaiveDateTime;
use chrono::prelude::*;
use event_store_derive_internals::EventData;
use event_store_derive_internals::Events;
use futures::{future, Future};
use lapin_futures::channel::Channel;
use log::{debug, error, trace};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use std::fmt;
use std::fmt::Debug;
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::tcp::TcpStream;
use tokio::timer::Delay;

#[derive(Clone)]
pub struct SubscribableStore {
    store_namespace: String,
    channel: Channel<TcpStream>,
    inner_store: Store,
}

impl Debug for SubscribableStore {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SubscribableStore namespace {}", self.store_namespace)
    }
}

impl SubscribableStore {
    pub fn new(
        store_namespace: String,
        pool: Pool<PostgresConnectionManager>,
    ) -> impl Future<Item = Self, Error = io::Error> {
        let addr: SocketAddr = "127.0.0.1:5673".parse().unwrap();

        // TODO: Pass in an AMQP adapter inside a promise instead of doing this here
        amqp_connect(addr, "test_exchange".into())
            .map(|channel| Self {
                channel: channel.clone(),
                store_namespace: store_namespace.clone(),
                inner_store: Store::new(store_namespace, pool, channel),
            })
            .and_then(|store| {
                debug!("Begin listening for event replay requests");

                store
                    .subscribe_no_replay::<EventReplayRequested>()
                    .map(|_| store)
            })
            // FIXME: Remove this delay
            .and_then(|store| {
                // Give the replay consumer some time to settle
                Delay::new(Instant::now() + Duration::from_millis(100))
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "wait error"))
                    .map(|_| store)
            })
    }

    pub fn aggregate<T, QA, E>(&self, query_args: QA) -> impl Future<Item = T, Error = io::Error>
    where
        E: Events,
        T: Aggregator<E, QA, PgQuery>,
        QA: Clone + Debug,
    {
        self.inner_store.aggregate(query_args)
    }

    pub fn save<ED>(&self, event: Event<ED>) -> impl Future<Item = Event<ED>, Error = io::Error>
    where
        ED: EventData + Debug,
    {
        self.inner_store.save(event)
    }

    pub fn save_no_emit<ED>(
        &self,
        event: Event<ED>,
    ) -> impl Future<Item = Event<ED>, Error = io::Error>
    where
        ED: EventData + Debug,
    {
        self.inner_store.save_no_emit(event)
    }

    fn subscribe_no_replay<ED>(&self) -> impl Future<Item = (), Error = io::Error>
    where
        ED: EventHandler + Debug + Send + 'static,
    {
        let queue_name = self.namespaced_event_queue_name::<ED>();
        let inner_store = self.inner_store.clone();

        debug!("Begin listening for events on queue {}", queue_name);

        let consumer = amqp_create_consumer(
            self.channel.clone(),
            queue_name,
            "test_exchange".into(),
            move |event: Event<ED>| {
                // TODO: Save event in this closure somewhere

                ED::handle_event(event, &inner_store);
            },
        );

        tokio::spawn(consumer.map_err(|e| {
            error!("Consumer error: {}", e);

            ()
        }));

        future::ok(())
    }

    pub fn subscribe<ED>(&self) -> impl Future<Item = (), Error = io::Error>
    where
        ED: EventHandler + Debug + Send + 'static,
    {
        let replay_queue_name = self.event_queue_name::<EventReplayRequested>();
        let inner_channel = self.channel.clone();

        self.subscribe_no_replay::<ED>();

        self.inner_store
            .last_event::<ED>()
            .map(|last_event| {
                trace!("Fetched last event {:?}", last_event);

                last_event.map(|ev| ev.context.time).unwrap_or_else(|| {
                    DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc)
                })
            })
            .and_then(move |since| {
                trace!("Emit replay request for events since {:?}", since);

                amqp_emit_event(
                    inner_channel,
                    replay_queue_name,
                    "test_exchange".into(),
                    EventReplayRequested::from_event::<ED>(since),
                )
            })
            // FIXME: Remove this delay
            .and_then(|_| {
                // Give the consumer some time to settle
                Delay::new(Instant::now() + Duration::from_millis(100))
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "wait error"))
                    .map(|_| ())
            })
    }

    fn namespaced_event_queue_name<ED>(&self) -> String
    where
        ED: EventData,
    {
        format!("{}-{}", self.store_namespace, self.event_queue_name::<ED>())
    }

    fn event_queue_name<ED>(&self) -> String
    where
        ED: EventData,
    {
        format!("{}.{}", ED::event_namespace(), ED::event_type())
    }
}