use nostr_sdk::{
    ClientMessage,
    Filter,
    Kind,
    RelayPoolNotification,
};

use nostr_sdk::relay::{
    pool::RelayPool,
    RelayPoolOptions,
    RelayOptions,
};

use std::collections::{
    btree_map::BTreeMap,
};

use std::io::{
    stdout,
};

use std::fmt;

use tokio::signal;

fn sanitize_string(s: &str) -> String {
    s.chars()
        .map(|c|
            // Kinda silly to make a match? seems cleaner anyway somehow
            match c {
                // whitespace are technically also control chars
                c if c.is_whitespace() => { c }
                c if c.is_control() => { char::REPLACEMENT_CHARACTER }
                c => { c }
            }
        )
        .collect()
}

struct KindStats {
    pub seen: u64,
}

impl KindStats {
    pub fn seen(&mut self) -> u64 {
        self.seen += 1;

        self.seen
    }
}

impl Default for KindStats {
    fn default() -> Self {
        KindStats { seen: 0 }
    }
}

impl fmt::Display for KindStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "seen: {}", self.seen)
    }
}

#[tokio::main]
async fn main() {
    let pool_options = RelayPoolOptions::default();

    let pool = RelayPool::new(pool_options);

    let relay_options = RelayOptions::default();

    pool.add_relay("wss://relay.damus.io", None, relay_options)
        .await
        .expect("add damus relay");

    let mut filters: Vec<Filter> = Vec::new();
    let mut filter = Filter::new();
    //filter.kinds.insert(1.into());
    filters.push(filter);
    pool.subscribe(filters, None).await;

    pool.connect(true).await;

    let mut kind_stats: BTreeMap<Kind, KindStats> = BTreeMap::new();

    let mut notifications = pool.notifications();

    loop {
        tokio::select! {
            notification = notifications.recv() => {
                match notification {
                    Ok(RelayPoolNotification::Event(url, event)) => {
                        if let Some(stats) = kind_stats.get_mut(&event.kind) {
                            stats.seen();
                        } else {
                            let mut stats = KindStats::default();
                            stats.seen();

                            kind_stats.insert(event.kind, stats);
                        }

                        let kind: u64 = event.kind.into();
                        println!("event kind {kind}!");
                    },
                    Ok(RelayPoolNotification::Message(..)) |
                    Ok(RelayPoolNotification::RelayStatus{..}) => {
                        //println!("message or relay status");
                    },
                    Ok(RelayPoolNotification::Stop) => {
                        println!("stop!");
                        break;
                    },
                    Ok(RelayPoolNotification::Shutdown) => {
                        eprintln!("shutdown!");
                        break;
                    },
                    Err(e) => {
                        eprintln!("error! {e}");
                    }
                }
            }
            _ = signal::ctrl_c() => {
                break;
            }
        }
    }

    for (kind, stats) in kind_stats.iter() {
        println!("Kind {kind} => {stats}");
    }
}
