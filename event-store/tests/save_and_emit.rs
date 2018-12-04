#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate event_store_derive;
extern crate event_store;
extern crate pretty_env_logger;

use event_store::Event;
use event_store::*;
use futures::future::{self, Future};
use std::net::SocketAddr;
use tokio_core::reactor::Core;

#[derive(EventData, Debug)]
#[event_store(namespace = "some_namespace")]
pub struct TestEvent {
    pub num: i32,
}

#[test]
fn save_and_emit() {
    pretty_env_logger::init();

    let addr: SocketAddr = "127.0.0.1:5673".parse().unwrap();
    let test_event = TestEvent { num: 100 };

    trace!("Save and emit test");

    // let mut core = Core::new().unwrap();
    // let _handle = core.handle();

    let run = amqp_connect(addr, "test_exchange".into())
        .and_then(move |channel| {
            info!("AMQP connected");

            let consumer = amqp_create_consumer(
                channel.clone(),
                "rando_queue".into(),
                "test_exchange".into(),
                |ev: Event<TestEvent>| {
                    debug!("Received event {}", ev.id);
                },
            );

            tokio::spawn(consumer.map_err(|_| ()));

            amqp_emit_event(
                channel.clone(),
                "rando_queue".into(),
                "test_exchange".into(),
                &Event::from_data(test_event),
            )
        })
        .map(|_| ())
        .map_err(|_| ());

    // core.run(run).unwrap();
    tokio::run(run);
}