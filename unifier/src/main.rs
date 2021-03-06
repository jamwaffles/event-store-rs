#[macro_use]
extern crate log;

mod config;
mod event;

use config::{Config, ConfigConnection};
use event::Event;
use pbr::ProgressBar;
use postgres::{Client, NoTls};
use std::collections::HashMap;
use structopt::StructOpt;
use uuid::Uuid;

#[derive(Debug, StructOpt)]
#[structopt(name = "unify", about = "Unify multiple event stores into one")]
struct Options {
    /// Which connection to select from connections.toml
    #[structopt(short = "c", long = "connection")]
    connection: String,

    /// Truncate destination events table before inserting data
    #[structopt(long = "truncate-dest")]
    truncate_dest: bool,

    /// Execute a copy from one events database into another
    #[structopt(long = "copy")]
    copy: bool,
}

fn collect_domain_events(
    domain: &str,
    namespace: &str,
    connection: &ConfigConnection,
) -> Result<Vec<Event>, String> {
    info!(
        "Collecting events from domain {} where namespace is {}...",
        domain, namespace
    );

    let mut conn = Client::connect(&format!("{}/{}", connection.source_db_uri, domain), NoTls)
        .map_err(|e| e.to_string())?;

    // NOTE: This query reformats dates to be RFC3339 compatible
    conn.query(
        r#"select
            id,
            data,
            context || jsonb_build_object('time', to_timestamp(context->>'time', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) as context
        from events where data->>'event_namespace' = $1 order by context->>'time' asc"#,
        &[&namespace],
    )
    .map_err(|e| e.to_string())
    .map(|result| {
        info!("Collected {} events for namespace {} in domain {}", result.len(), namespace, domain);

        result.iter().map(|row| {
            let id: Uuid = row.get(0);
            let data: serde_json::Value = row.get(1);
            let context: serde_json::Value = row.get(2);

            Event {
                id,
                data: serde_json::from_value(data).expect(&format!("Failed to parse event data for event ID {} (domain {})", id, domain)),
                context: serde_json::from_value(context).expect(&format!("Failed to parse event context for event ID {} (domain {})", id, domain)),
            }
        }).collect()
    })
}

fn collect_store_events(connection: &ConfigConnection) -> Result<HashMap<Uuid, Event>, String> {
    info!("Collecting all events from event-store DB",);

    let mut conn = Client::connect(&format!("{}/event-store", connection.source_db_uri), NoTls)
        .map_err(|e| e.to_string())?;

    // NOTE: This query reformats dates to be RFC3339 compatible
    conn.query(
        r#"select
            id,
            data,
            context || jsonb_build_object('time', to_timestamp(context->>'time', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) as context
        from events order by context->>'time' asc"#,
        &[],
    )
    .map_err(|e| e.to_string())
    .map(|result| {
        let mut pb = ProgressBar::new(result.len() as u64);
        pb.set_width(Some(100));
        pb.message("Parsing event ");

        info!("Collected {} events from event store", result.len());

        result.iter().map(|row| {
            let id: Uuid = row.get(0);
            let data: serde_json::Value = row.get(1);
            let context: serde_json::Value = row.get(2);

            let data = serde_json::from_value(data).expect(&format!("Failed to parse event data for event ID {}", id));
            let context = serde_json::from_value(context).expect(&format!("Failed to parse event context for event ID {} ", id));

            pb.inc();

            (id, Event {
                id,
                data,
                context,
            })
        }).collect()
    })
}

fn main() -> Result<(), String> {
    pretty_env_logger::init();

    let config = Config::load()?;

    let args = Options::from_args();

    debug!("Args {:?}", args);

    let connection = config.get(&args.connection).expect(&format!(
        "Failed to find config for key {}",
        args.connection
    ));

    debug!("Connection {:?}", connection);

    let all_events: HashMap<Uuid, Event> = if !args.copy {
        let mut collect_pb = ProgressBar::new(connection.domains.len() as u64);
        collect_pb.set_width(Some(100));
        collect_pb.message("Collecting events... ");

        let domain_events: Vec<Vec<Event>> = connection
            .domains
            .iter()
            .map(|(domain, namespace)| {
                collect_pb.message(&format!("Collecting {} ", domain));
                collect_pb.inc();

                collect_domain_events(domain, namespace, connection).unwrap()
            })
            .collect();

        let domain_events_sum: usize = domain_events.iter().map(|events| events.len()).sum();

        debug!("Collected total {} events", domain_events_sum);

        collect_pb.finish_println(&format!("Collected total {} events", domain_events_sum));

        let all_events: HashMap<Uuid, Event> = domain_events
            .into_iter()
            .flat_map(|events| events.into_iter().map(|event| (event.id, event)))
            .collect();

        if all_events.len() != domain_events_sum {
            error!("Unique list of events is not the same length as all collected events");
            error!(
                "Expected all events {} to equal domain events {}",
                all_events.len(),
                domain_events_sum
            );

            return Err(String::from("Events are not properly unique"));
        }

        all_events
    } else {
        println!("Collecting all events from event_store DB...");

        collect_store_events(&connection)?
    };

    let all_events_len = all_events.len();

    println!("Collected {} events", all_events_len);

    let mut dest_connection =
        Client::connect(&connection.dest_db_uri.clone(), NoTls).map_err(|e| e.to_string())?;

    let mut txn = dest_connection.transaction().map_err(|e| e.to_string())?;
    let stmt = txn
        .prepare(
            "insert into events (id, data, context) values ($1, $2, $3)
            on conflict (id) do update set
            data = excluded.data,
            context = excluded.context",
        )
        .map_err(|e| e.to_string())?;

    if args.truncate_dest {
        warn!("Truncating destination table events");

        txn.simple_query("truncate table events")
            .map_err(|e| e.to_string())?;
    }

    info!("Writing {} events to destination...", all_events_len);

    let mut insert_pb = ProgressBar::new(all_events_len as u64);
    insert_pb.set_width(Some(100));
    insert_pb.message("Insert event ");

    for (id, event) in all_events.iter() {
        debug!("Insert event {}", id);

        txn.execute(
            &stmt,
            &[
                &event.id,
                &serde_json::to_value(&event.data).unwrap(),
                &serde_json::to_value(&event.context).unwrap(),
            ],
        )
        .map_err(|e| e.to_string())?;

        insert_pb.inc();
    }

    txn.commit().map_err(|e| e.to_string())?;

    info!("Successfully inserted {} events", all_events_len);

    insert_pb.finish_println(&format!(
        "Successfully inserted {} events\n",
        all_events_len
    ));

    Ok(())
}
