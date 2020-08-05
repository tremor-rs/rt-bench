use async_channel::{bounded, Receiver, Sender};
use chrono::{Timelike, Utc};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::fs::File;
use std::io::{BufRead, Read};
use std::path::Path;
use std::time::Duration;
use xz2::read::XzDecoder;

#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

const QSIZE: usize = 1024 * 4;
const RUN_LEN: u64 = Duration::from_secs(60).as_nanos() as u64;
const SETUP: u64 = Duration::from_secs(5).as_nanos() as u64;
const FILE: &str = "./data.json.xz";

#[allow(clippy::cast_sign_loss)]
pub fn nanotime() -> u64 {
    let now = Utc::now();
    let seconds: u64 = now.timestamp() as u64;
    let nanoseconds: u64 = u64::from(now.nanosecond());

    (seconds * 1_000_000_000) + nanoseconds
}

struct Blaster {
    data: Vec<Vec<u8>>,
    idx: usize,
    tx: Sender<Event>,
}

impl Blaster {
    fn from_file(file: &str, tx: Sender<Event>) -> Self {
        let mut source_data_file = File::open(&file).unwrap();

        let mut data = vec![];
        let ext = Path::new(file).extension().map(std::ffi::OsStr::to_str);
        if ext == Some(Some("xz")) {
            XzDecoder::new(source_data_file)
                .read_to_end(&mut data)
                .unwrap();
        } else {
            source_data_file.read_to_end(&mut data).unwrap();
        };

        Self {
            idx: 0,
            data: data
                .lines()
                .map(|l| l.unwrap().as_bytes().to_vec())
                .collect(),
            tx,
        }
    }
    async fn run(mut self) {
        loop {
            if let Some(data) = self.data.get(self.idx % self.data.len()) {
                let mut data = data.clone();
                self.tx
                    .send(Event {
                        data: simd_json::to_owned_value(data.as_mut_slice()).expect("Bad JSON"),
                        ingest_ns: nanotime(),
                    })
                    .await
                    .expect("failed to send")
            } else {
                panic!("No data")
            }
            self.idx += 1
        }
    }
}

struct Event {
    ingest_ns: u64,
    data: OwnedValue,
}

async fn pipeline(rx: Receiver<Event>, tx: Sender<Event>) {
    while let Ok(mut e) = rx.recv().await {
        // do some uselss data manipulation to simulate processing

        let len = e
            .data
            .get("about")
            .and_then(ValueTrait::as_str)
            .map(|s| s.len())
            .unwrap_or_default();
        e.data.insert("len", len).unwrap();
        tx.send(e).await.unwrap();
    }
}

#[async_std::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let concurrency: usize = if let Some(first) = args.get(1) {
        first.parse().expect("Valid concurrency")
    } else {
        1
    };
    let (main_tx, main_rx) = bounded(QSIZE);

    let mut blasters = Vec::new();
    for _ in 0..concurrency {
        let (tx, rx) = bounded(QSIZE);
        blasters.push((rx, Blaster::from_file(FILE, tx.clone())));
    }

    for (rx, blaster) in blasters.drain(..) {
        let tx = main_tx.clone();
        async_std::task::spawn(async move { pipeline(rx, tx).await });
        async_std::task::spawn(async move { blaster.run().await });
    }

    let start_ns = nanotime();
    let mut throughput: usize = 0;
    let mut v: Vec<u8> = Vec::with_capacity(1024);
    while let Ok(Event { ingest_ns, data }) = main_rx.recv().await {
        let d = ingest_ns.saturating_sub(start_ns);
        if d > RUN_LEN + SETUP {
            println!(
                "{} {:.1} MB/s",
                concurrency,
                (throughput as f64 / Duration::from_nanos(RUN_LEN).as_secs_f64())
                    / (1024.0 * 1024.0)
            );
            std::process::exit(0);
        } else if d > SETUP {
            data.write(&mut v).expect("Bad json");
            throughput += v.len();
            v.clear();
        }
    }
}
