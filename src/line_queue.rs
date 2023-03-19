use std::collections::VecDeque;

use anyhow::Result;

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
    min_buf_len: usize,
    repos: VecDeque<Repo>,
    lines: VecDeque<SrcString>,
    page_no: u32,
    _trace_guard: ta::non_blocking::WorkerGuard,
}

impl LineQueue {
    pub async fn new(min_buf_len: usize) -> Result<Self> {
        let _trace_guard = init_log("rt_log")?;
        let line_queue = Self {
            min_buf_len,
            repos: VecDeque::new(),
            lines: VecDeque::new(),
            page_no: 1,
            _trace_guard,
        };
        Ok(line_queue.init().await?)
    }

    async fn init(mut self) -> Result<Self> {
        self.fill().await?;
        Ok(self)
    }

    pub fn next_line(&mut self) -> Option<SrcString> {
        self.lines.pop_front()
    }

    pub async fn fill(&mut self) -> Result<()> {
        while self.lines.len() < self.min_buf_len {
            match self.repos.pop_front() {
                Some(mut repo) => {
                    while self.lines.len() < self.min_buf_len {
                        if let Some(line) = repo.get_line().await? {
                            self.lines.push_back(line);
                        } else {
                            break;
                        }
                    }
                    if self.lines.len() >= self.min_buf_len {
                        self.repos.push_front(repo);
                    }
                },
                None => {
                    // fetch more repos
                    self.repos.extend(get_repo_urls(self.page_no).await?);
                    self.page_no += 1;
                },
            }
        }
        Ok(())
    }
}

#[tokio::main]
pub async fn dump() -> Result<()> {
    let mut line_queue = LineQueue::new(10).await?;
    for _ in 0..100 {
        let line = loop {
            match line_queue.next_line() {
                Some(x) => break x,
                None => {
                    line_queue.fill().await?;
                },
            }
        };

        println!("{} ::: {}",line.string, line.source);
    }
    Ok(())
}
