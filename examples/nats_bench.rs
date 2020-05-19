use std::{
    num::NonZeroUsize,
    sync::{Arc, Barrier},
    thread,
    time::Instant,
};

use nats;
use rand::{thread_rng, Rng};
use structopt::StructOpt;

/// Simple NATS bench tool
#[derive(Debug, StructOpt)]
struct Args {
    /// The nats server URLs (separated by comma) (default "nats://127.0.0.1:4222")
    #[structopt(long, short, default_value = "nats://127.0.0.1:4222")]
    url: String,

    /// User Credentials File
    #[structopt(long = "creds")]
    creds: Option<String>,

    /// Size of the message. (default 128)
    #[structopt(long, default_value = "128")]
    message_size: usize,

    /// Number of Messages to Publish (default 100000)
    #[structopt(long, short, default_value = "100000")]
    number_of_messages: NonZeroUsize,

    /// Number of Concurrent Publishers (default 1)
    #[structopt(short, long, default_value = "1")]
    publishers: NonZeroUsize,

    /// Number of Concurrent Subscribers
    #[structopt(short = "s", long, default_value = "0")]
    subscribers: usize,

    /// Use TLS Secure Connection
    #[structopt(short = "tls")]
    tls: bool,

    /// The subject to use
    subject: String,
}

fn main() -> std::io::Result<()> {
    let args = Args::from_args();
    let opts = nats::ConnectionOptions::new()
        .with_name("nats_bench rust client")
        .tls_required(args.tls);

    let nc = if let Some(creds_path) = args.creds {
        opts.with_credentials(creds_path).connect(&args.url)?
    } else {
        opts.connect(&args.url)?
    };

    let messages = if args.number_of_messages.get() % args.publishers.get() != 0 {
        let bumped_idx = (args.number_of_messages.get() / args.publishers.get()) + 1;
        bumped_idx * args.publishers.get()
    } else {
        args.number_of_messages.get()
    };

    let message_size = args.message_size;

    let barrier = Arc::new(Barrier::new(1 + args.publishers.get() + args.subscribers));

    let mut threads = vec![];

    for _ in 0..args.publishers.get() {
        let barrier = barrier.clone();
        let nc = nc.clone();
        let subject = args.subject.clone();
        threads.push(thread::spawn(move || {
            let mut rng = thread_rng();
            let msg: String = (0..message_size).map(|_| rng.gen::<char>()).collect();
            barrier.wait();
            for _ in 0..messages {
                nc.publish(&subject, &msg).unwrap();
            }
        }));
    }

    for _ in 0..args.subscribers {
        let barrier = barrier.clone();
        let nc = nc.clone();
        let subject = args.subject.clone();
        threads.push(thread::spawn(move || {
            barrier.wait();
            let s = nc.subscribe(&subject).unwrap();
            for _ in 0..messages {
                s.next().unwrap();
            }
        }));
    }

    println!(
        "Starting benchmark [msgs={}, msgsize={}, pubs={}, subs={}",
        messages,
        args.message_size,
        args.publishers.get(),
        args.subscribers
    );

    barrier.wait();

    let start = Instant::now();

    for thread in threads.into_iter() {
        thread.join().unwrap();
    }

    let end = start.elapsed();

    let millis = std::cmp::max(1, end.as_millis() as u64);
    let frequency = 1000 * messages as u64 / millis;
    let mbps = (args.message_size * messages) as u64 / millis / 1024;

    println!(
        "duration: {:?} frequency: {} mbps: {}",
        end, frequency, mbps
    );

    Ok(())
}