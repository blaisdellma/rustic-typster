use std::collections::VecDeque;

use anyhow::{Result,bail};
use reqwest::{Client,StatusCode};
use select::{document::Document,predicate::{Predicate,Attr,Name}};

use tracing::{info,debug,warn,error,Level};
use tracing_subscriber::{self as ts, EnvFilter};
use tracing_appender as ta;

use crate::SrcString;

const BASE_CRATES_URL: &str = "https://crates.io/api/v1/crates?sort=recent-downloads";

static APP_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

fn init_log(prefix: &str) -> Result<ta::non_blocking::WorkerGuard> {
    let log_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let (file, guard) = ta::non_blocking(ta::rolling::daily(log_dir,prefix));
    ts::fmt()
        .with_writer(file)
        .with_max_level(Level::DEBUG)
        .with_env_filter({
            EnvFilter::from_default_env()
                .add_directive("warn".parse()?)
                .add_directive("rustic_typster=debug".parse()?)
        }).init();
    debug!("Log init successful");
    Ok(guard)
}

#[derive(Debug)]
pub struct LineGenerator {
    min_buf_len: usize,
    repo_urls: VecDeque<SrcString>,
    file_urls: VecDeque<SrcString>,
    lines: VecDeque<SrcString>,
    page_no: u32,
    _trace_guard: ta::non_blocking::WorkerGuard,
}

impl LineGenerator {
    pub async fn new(min_buf_len: usize) -> Result<Self> {
        let _trace_guard = init_log("rt_log")?;
        let line_gen = LineGenerator {
            min_buf_len,
            repo_urls: VecDeque::new(),
            file_urls: VecDeque::new(),
            lines: VecDeque::new(),
            page_no: 1,
            _trace_guard,
        };
        Ok(line_gen.init().await?)
    }

    async fn init(mut self) -> Result<Self> {
        self.extend().await?;
        Ok(self)
    }

    pub fn next_line(&mut self) -> Option<SrcString> {
        self.lines.pop_front()
    }

    pub async fn extend(&mut self) -> Result<()> {
        while self.lines.len() < self.min_buf_len {
            self.extend_lines().await?;
        }
        Ok(())
    }

    async fn extend_repos(&mut self) -> Result<usize> {
        let mut to_add = Vec::new();
        while to_add.len() == 0 {
            to_add = get_repo_urls(self.page_no).await?;
            self.page_no += 1;
        }
        self.repo_urls.extend(to_add);
        Ok(self.repo_urls.len())
    }

    async fn extend_files(&mut self) -> Result<usize> {
        let mut to_add = Vec::new();
        while to_add.len() == 0 {
            if self.repo_urls.len() == 0 {
                self.extend_repos().await?;
            }
            let repo_url = self.repo_urls.pop_front().unwrap();
            let (to_add_files,to_add_folders) = get_file_urls(repo_url).await?;
            self.repo_urls.extend(to_add_folders);
            to_add.extend(to_add_files);
        }
        self.file_urls.extend(to_add);
        Ok(self.file_urls.len())
    }

    async fn extend_lines(&mut self) -> Result<usize> {
        let mut to_add = Vec::new();
        while to_add.len() == 0 {
            if self.file_urls.len() == 0 {
                self.extend_files().await?;
            }
            let file_url = self.file_urls.pop_front().unwrap();
            to_add = get_lines(file_url).await?;
        }
        self.lines.extend(to_add);
        Ok(self.lines.len())
    }

}

async fn get_page_contents(url: String) -> Result<String> {
    let client = Client::builder().user_agent(APP_USER_AGENT).build()?;
    let response = client.get(&url).send().await?;
    if response.status() != StatusCode::OK {
        bail!("Error Code: {} trying to fetch {}",response.status(),url);
    }
    let text = response.text().await?;
    Ok(text)
}

async fn get_repo_urls(page_no: u32) -> Result<Vec<SrcString>> {
    debug!("Fetching repo urls from page {}", page_no);
    let url = format!("{}&page={}",BASE_CRATES_URL,page_no);
    let json_str = get_page_contents(url).await?;
    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
    let mut results = Vec::new();
    if let serde_json::Value::Array(v) = &json_val["crates"] {
        for crat in v {
            if let serde_json::Value::String(repo_url) = &crat["repository"] {
                if repo_url.contains("github.com") {
                    if let serde_json::Value::String(id) = &crat["id"] {
                        results.push(SrcString {
                            string: repo_url.into(),
                            source: id.into(),
                        });
                    }
                }
            }
        }
    }
    if results.len() == 0 {
        bail!("No crates found!");
    }
    Ok(results)
}

async fn get_file_urls(repo_url: SrcString) -> Result<(Vec<SrcString>,Vec<SrcString>)> {
    let source = repo_url.source;
    let repo_url = repo_url.string;
    let mut file_urls = Vec::new();
    let mut folder_urls = Vec::new();

    let contents = get_page_contents(repo_url).await?;
    let document = Document::from(contents.as_str());

    for node in document.find(Attr("role","rowheader").descendant(Name("a"))) {
        match node.attr("rel") {
            Some(_) => (),
            None => {
                match node.attr("href") {
                    None => (),
                    Some(s) => {
                        if s.contains("blob") && s.ends_with(".rs") {
                            file_urls.push(SrcString {
                                string: format!("https://github.com{}",s),
                                source: source.clone(),
                            });
                        } else if s.contains("tree") {
                            folder_urls.push(SrcString {
                                string: format!("https://github.com{}",s),
                                source: source.clone(),
                            });
                        }
                    }
                }
            },
        }
    }


    Ok((file_urls,folder_urls))
}

async fn convert_file_url_to_raw(file_url: SrcString) -> Result<Option<SrcString>> {
    let source = file_url.source;
    let file_url = file_url.string;
    let contents = get_page_contents(file_url).await?;
    let document = Document::from(contents.as_str());
    Ok(match document.find(Attr("id","raw-url")).next() {
        None => None,
        Some(x) => match x.attr("href") {
            None => None,
            Some(x) => {
                Some({
                    SrcString {
                        string: format!("https://github.com{}",x),
                        source,
                    }
                })
            },
        },
    })
}

async fn get_lines(file_url: SrcString) -> Result<Vec<SrcString>> {
    debug!("Fetching lines from file: {}", file_url.string);
    let source = file_url.source.clone();
    let raw_url = match convert_file_url_to_raw(file_url).await? {
        Some(x) => x,
        None => return Ok(Vec::new()),
    };
    let contents = get_page_contents(raw_url.string).await?;
    let lines: Vec<_> = contents.split_terminator("\n").map(|s| s.trim()).filter(|s| {
        s.len() >= 10 && s.len() <= 80
        &&
        !s.starts_with("//")
    }).map(|s| {
        debug!("Found line: \"{}\" from {}",s,source);
        SrcString{
            string: s.into(),
            source: source.clone(),
        }
    }).collect();
    Ok(lines)
}

#[tokio::main]
pub async fn main_rustic_typster() -> Result<()> {
    let mut line_gen = LineGenerator::new(10).await?;
    for _ in 0..100 {
        let line = loop {
            match line_gen.next_line() {
                Some(x) => break x,
                None => {
                    line_gen.extend().await?;
                },
            }
        };

        println!("{} ::: {}",line.string, line.source);
    }
    Ok(())
}

pub fn start_rustic_typster() {
    println!("Running Rustic Typster");
    main_rustic_typster().unwrap();
    println!("Done");
}
