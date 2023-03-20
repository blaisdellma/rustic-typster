use std::collections::VecDeque;

use anyhow::{bail,Result};

use tokio::sync::mpsc::{channel,Sender,WeakSender};
use tokio::task::JoinHandle;

use tracing::{debug,Level};
use tracing_subscriber::{self as ts, EnvFilter};
use tracing_appender as ta;

use crate::fetch::*;

#[derive(Debug)]
pub struct SrcString {
    pub string: String,
    pub source: String,
}

impl Default for SrcString {
    fn default() -> Self {
        Self {
            string: "Waiting on line ...".into(),
            source: "".into(),
        }
    }
}

fn init_log(prefix: &str) -> Result<ta::non_blocking::WorkerGuard> {
    let log_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let (file, guard) = ta::non_blocking(ta::rolling::daily(log_dir,prefix));
    ts::fmt()
        .with_writer(file)
        .with_max_level(Level::TRACE)
        .with_env_filter({
            EnvFilter::from_default_env()
                .add_directive("warn".parse()?)
                .add_directive("rustic_typster=trace".parse()?)
        }).init();
    debug!("Log init successful");
    Ok(guard)
}

#[derive(Debug)]
pub struct LineQueue {
    repos: VecDeque<Repo>,
    weak_tx: WeakSender<SrcString>,
    page_no: u32,
    _trace_guard: ta::non_blocking::WorkerGuard,
}

impl LineQueue {
    pub fn new(tx: Sender<SrcString>) -> JoinHandle<Result<()>> {
        tokio::spawn( async move {
            let _trace_guard = init_log("rt_log")?;
            let line_queue = Self {
                repos: VecDeque::new(),
                weak_tx: tx.downgrade(),
                page_no: 1,
                _trace_guard,
            };
            line_queue.init().await
        })
    }

    async fn init(mut self) -> Result<()> {
        loop {
            let line = match self.repos.pop_front() {
                Some(mut repo) => {
                    if let Some(line) = repo.get_line().await? {
                        self.repos.push_front(repo);
                        Some(line)
                    } else {
                        None
                    }
                },
                None => {
                    // fetch more repos
                    self.repos.extend(get_repo_urls(self.page_no).await?);
                    self.page_no += 1;
                    None
                },
            };
            if let Some(line) = line {
                if let Some(tx) = self.weak_tx.clone().upgrade() {
                    let permit = tx.reserve().await;
                    if tx.is_closed() { break; }
                    permit?.send(line);
                } else {
                    break;
                }
            }
        }
        Ok(())
    }
}

#[tokio::main]
pub async fn dump() -> Result<()> {
    let (tx, mut rx) = channel::<SrcString>(10);
    let line_queue = LineQueue::new(tx.clone());
    for _ in 0..100 {
        let line = match rx.recv().await {
            Some(x) => x,
            None => bail!("channel is closed"),
        };
        println!("{} ::: {}",line.string, line.source);
    }
    rx.close();
    line_queue.await?
}
