#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

use serde::Deserialize;
use std::fmt::Debug;
use std::marker::PhantomData;

// --- Event store crate ---
trait Event {}
trait Events {}
trait StoreQuery {}
struct PgQuery(String);
trait Aggregator<E: Events, A, Q: StoreQuery>: Copy + Clone + Debug + Default {
    fn apply_event(acc: Self, event: &E) -> Self;

    fn query() -> Q;
}

impl StoreQuery for PgQuery {}

trait Store<E: Events> {
    fn aggregate<T, A, Q: StoreQuery>(&self, query: A) -> T
    where
        E: Events,
        T: Aggregator<E, A, Q>;
}

struct FakeBackedStore<E: Events> {
    phantom: PhantomData<E>,
}

impl<'a, E> Store<E> for FakeBackedStore<E>
where
    E: Events + Deserialize<'a>,
{
    fn aggregate<T, A, Q: StoreQuery>(&self, _query: A) -> T
    where
        T: Aggregator<E, A, Q>,
    {
        let inc: E = serde_json::from_str(
            r#"{
            "type": "Inc",
            "by": 1
        }"#,
        ).unwrap();
        let dec: E = serde_json::from_str(
            r#"{
            "type": "Dec",
            "by": 1
        }"#,
        ).unwrap();

        let events = vec![inc, dec];

        let result = events
            .iter()
            .fold(T::default(), |acc, event| T::apply_event(acc, event));

        result
    }
}

// --- Example implementation for the Toaster domain ---
#[derive(Deserialize)]
struct Increment {
    pub by: i32,
}
#[derive(Deserialize)]
struct Decrement {
    pub by: i32,
}

impl Event for Increment {}
impl Event for Decrement {}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ToasterEvents {
    Inc(Increment),
    Dec(Decrement),
}

impl Events for ToasterEvents {}

#[derive(Debug, Copy, Clone, PartialEq)]
struct SomeDomainEntity {
    counter: i32,
}

impl Default for SomeDomainEntity {
    fn default() -> Self {
        Self { counter: 0 }
    }
}

impl Aggregator<ToasterEvents, (String, String), PgQuery> for SomeDomainEntity {
    fn apply_event(acc: Self, event: &ToasterEvents) -> Self {
        let counter = match event {
            ToasterEvents::Inc(inc) => acc.counter + inc.by,
            ToasterEvents::Dec(dec) => acc.counter - dec.by,
        };

        Self { counter, ..acc }
    }

    fn query() -> PgQuery {
        PgQuery(String::from("SELECT * FROM events WHERE toast = 'yes'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_aggregates_events() {
        let events = vec![
            ToasterEvents::Inc(Increment { by: 1 }),
            ToasterEvents::Inc(Increment { by: 1 }),
            ToasterEvents::Dec(Decrement { by: 2 }),
            ToasterEvents::Inc(Increment { by: 2 }),
            ToasterEvents::Dec(Decrement { by: 3 }),
            ToasterEvents::Dec(Decrement { by: 3 }),
        ];

        let result: SomeDomainEntity = events
            .iter()
            .fold(SomeDomainEntity::default(), SomeDomainEntity::apply_event);

        assert_eq!(result, SomeDomainEntity { counter: -4 });
    }

    #[test]
    fn thing() {
        let store = FakeBackedStore::<ToasterEvents> {
            phantom: PhantomData,
        };

        let _entity: SomeDomainEntity = store.aggregate(("id".into(), "oenebtio".into()));
    }
}
