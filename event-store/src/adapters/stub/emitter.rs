//! Stub emitter implementation

use adapters::EmitterAdapter;
use futures::future::ok;
use std::io::Error;
use utils::BoxedFuture;
use Event;
use EventData;
use Events;

/// Stub event emitter
pub struct StubEmitterAdapter {}

impl StubEmitterAdapter {
    /// Create a new emitter stub
    pub fn new() -> Self {
        Self {}
    }
}

impl EmitterAdapter for StubEmitterAdapter {
    fn emit<E: Events>(&self, _event: &Event<E>) -> BoxedFuture<(), Error> {
        Box::new(ok(()))
    }

    fn subscribe<'a, ED: EventData, H>(&self, _handler: H) -> BoxedFuture<'a, (), Error>
    where
        H: Fn(&Event<ED>) -> (),
    {
        Box::new(ok(()))
    }
}
