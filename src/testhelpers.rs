//! Test helpers. Do not use in application code.

use super::{Aggregator, Event, Events};
use pg::PgQuery;
use postgres::types::ToSql;

#[derive(Deserialize)]
/// Test event
pub struct TestIncrementEvent {
    /// Increment by this much
    pub by: i32,
}
#[derive(Deserialize)]
/// Test event
pub struct TestDecrementEvent {
    /// Decrement by this much
    pub by: i32,
}

impl Event for TestIncrementEvent {}
impl Event for TestDecrementEvent {}

#[derive(Deserialize)]
#[serde(tag = "type")]
/// Set of all events in the domain
pub enum TestEvents {
    /// Increment
    #[serde(rename = "some_namespace.Inc")]
    Inc(TestIncrementEvent),
    /// Decrement
    #[serde(rename = "some_namespace.Dec")]
    Dec(TestDecrementEvent),
    /// Some other event
    #[serde(rename = "some_namespace.Other")]
    Other,
}

impl Events for TestEvents {}

#[derive(Debug, Copy, Clone, PartialEq)]
/// Testing entity for a pretend domain
pub struct TestCounterEntity {
    /// Current counter value
    pub counter: i32,
}

impl Default for TestCounterEntity {
    fn default() -> Self {
        Self { counter: 0 }
    }
}

impl<'a> Aggregator<TestEvents, String, PgQuery<'a>> for TestCounterEntity {
    fn apply_event(acc: Self, event: &TestEvents) -> Self {
        let counter = match event {
            TestEvents::Inc(inc) => acc.counter + inc.by,
            TestEvents::Dec(dec) => acc.counter - dec.by,
            _ => acc.counter,
        };

        Self { counter, ..acc }
    }

    fn query(field: String) -> PgQuery<'a> {
        let mut params: Vec<Box<ToSql>> = Vec::new();

        params.push(Box::new(field));

        PgQuery::new(
            "select * from events where data->>'test_field' = $1",
            params,
        )
    }
}
