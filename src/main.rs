use clap::{
    Args,
    Parser,
    Subcommand,
};

use crossterm::event::{
    EventStream,
    Event,
    KeyCode,
    KeyEvent,
    KeyModifiers,
};

use crossterm::terminal::{
    disable_raw_mode,
    enable_raw_mode,
};

use futures::stream::{
    Stream,
};

use futures::StreamExt;

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

use std::{
    str,
};

use std::fmt;

struct RawTerm { }

impl RawTerm {
    pub fn new() -> Self {
        enable_raw_mode()
            .expect("Enable raw mode");

        Self { }
    }
}

impl Drop for RawTerm {
    fn drop(&mut self) {
        disable_raw_mode()
            .expect("Disable raw mode");
    }
}

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

#[derive(Parser)]
#[command( name = "nostail" )]
struct Arguments {
    #[arg( short = 'r', long = "relay" )]
    relays: Vec<String>,

    #[arg( short = 'k', long = "kind" )]
    kinds: Vec<u64>,

    #[arg( short = 's', long = "stats" , default_value = "false" )]
    stats: bool,

    #[arg( short = 'c', long = "content" , default_value = "false" )]
    content: bool,

    #[arg( short = 't', long = "show-tags", default_value = "false" )]
    show_tags: bool,
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
    let args = Arguments::parse();

    let pool_options = RelayPoolOptions::default();

    let pool = RelayPool::new(pool_options);

    let relay_options = RelayOptions::default();

    for relay_url in args.relays.iter() {
        pool.add_relay(&relay_url[..], None, relay_options.clone())
            .await
            .expect(format!("add relay \"{relay_url}\"").as_ref());
    }

    let mut filters: Vec<Filter> = Vec::new();
    let mut filter = Filter::new();
    for &kind in args.kinds.iter() {
        filter.kinds.insert(kind.into());
    }
    filters.push(filter);
    pool.subscribe(filters, None).await;

    pool.connect(true).await;

    let mut kind_stats: BTreeMap<Kind, KindStats> = BTreeMap::new();

    // pause event display
    let mut pause = false;

    let mut notifications = pool.notifications();

    // RAII handling of enable/disable raw term
    let _term = RawTerm::new();

    let mut term_events = EventStream::new();

    loop {
        tokio::select! {
            notification = notifications.recv() => {
                match notification {
                    Ok(RelayPoolNotification::Event(_url, event)) => {
                        if pause {
                            continue;
                        }

                        if let Some(stats) = kind_stats.get_mut(&event.kind) {
                            stats.seen();
                        } else {
                            let mut stats = KindStats::default();
                            stats.seen();

                            kind_stats.insert(event.kind, stats);
                        }

                        let kind: u64 = event.kind.into();
                        if args.content {
                            let content = sanitize_string(event.content.as_ref());
                            let printable_content = str::replace(content.as_ref(), "\n", "\r\n");
                            println!("Kind {kind} => {printable_content}\r");
                        } else {
                            println!("Kind {kind}\r");
                        }
                    },
                    Ok(RelayPoolNotification::Message(..)) => {
                    },
                    Ok(RelayPoolNotification::RelayStatus{..}) => {
                        println!("relay status\r");
                    },
                    Ok(RelayPoolNotification::Stop) => {
                        println!("stop!\r");
                        break;
                    },
                    Ok(RelayPoolNotification::Shutdown) => {
                        eprintln!("shutdown!\r");
                        break;
                    },
                    Err(e) => {
                        eprintln!("error! {e}\r");
                    }
                }
            }
            event = term_events.next() => {
                match event {
                    Some(Ok(Event::Key(KeyEvent { code: KeyCode::Char('p'), .. }))) => {
                        pause = !pause;

                        if pause {
                            println!("PAUSED\r");
                        } else {
                            println!("UNPAUSED\r");
                        }
                    },
                    Some(Ok(Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers, .. }))) => {
                        if modifiers.contains(KeyModifiers::CONTROL) {
                            break;
                        }
                    }
                    _ => { }
                }
            }
        }
    }

    if args.stats {
        for (kind, stats) in kind_stats.iter() {
            println!("Kind {kind} => {stats}\r");
        }
    }
}
