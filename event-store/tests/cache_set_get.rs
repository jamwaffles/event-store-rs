#![feature(await_macro, async_await)]
#![feature(arbitrary_self_types)]

use event_store::adapters::{CacheResult, PgCacheAdapter};
use event_store::internals::{backward, test_helpers::*};
use futures::future::Future;
use log::trace;
use std::io;
use tokio::runtime::Runtime;

#[test]
fn cache_set_get() {
    pretty_env_logger::init();

    let fut = backward(async {
        let test_entity = TestCounterEntity { counter: 100 };

        trace!("Save and emit test");

        let conn = pg_create_random_db(None);

        let cache = await!(PgCacheAdapter::new(conn.clone()))?;

        await!(cache.save("_test".into(), &test_entity))?;

        let res = await!(cache.read::<TestCounterEntity>("_test".into()))?;

        Ok(res)
    })
    // Required so Rust can figure out what type `E` is
    .map_err(|e: io::Error| e);

    let res: Option<CacheResult<TestCounterEntity>> =
        Runtime::new().unwrap().block_on(fut).unwrap();

    assert_eq!(res.unwrap().0, TestCounterEntity { counter: 100 });
}
