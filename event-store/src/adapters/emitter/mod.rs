use event::Event;
use event_store_derive_internals::EventData;
use futures::Future;
use serde_json::Value as JsonValue;
use std::io;
use std::thread::JoinHandle;
use utils::BoxedFuture;

mod amqp;
mod stub;

pub use self::amqp::AMQPEmitterAdapter;
pub use self::stub::StubEmitterAdapter;

/// Event emitter interface
pub trait EmitterAdapter: Clone + Send + 'static {
    /// Emit an event
    fn emit<E: EventData + Send>(&self, event: &Event<E>) -> BoxedFuture<(), ()>;

    /// Emit an event given a namespace, type and payload value
    ///
    /// The payload object should be a compelete event, i.e. should contain
    /// `{ "id": ..., "data": ..., "context": ... }`. `data.event_namespace` and `data.event_type`
    /// MUST match the `event_namespace` and `event_type` arguments exactly.
    fn emit_with_string_ident(
        &self,
        event_namespace: &str,
        event_type: &str,
        event: &JsonValue,
    ) -> BoxedFuture<(), ()>;

    /// Subscribe to an event
    fn subscribe<ED, H>(&self, handler: H) -> BoxedFuture<(), ()>
    where
        ED: EventData + 'static,
        H: Fn(Event<ED>) -> () + Send + 'static;
}
