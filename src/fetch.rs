use std::collections::VecDeque;

use anyhow::{anyhow,bail,Result};
use async_recursion::async_recursion;
use async_trait::async_trait;
use reqwest::{Client,StatusCode};
use select::{document::Document,predicate::{Predicate,Attr,Name}};
use tracing::{debug,trace};

use crate::line_queue::SrcString;

const BASE_CRATES_URL: &str = "https://crates.io/api/v1/crates?sort=recent-downloads";
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"),"/",env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub enum UrlResource<T> {
    Resource(T),
    Url(String),
}

impl<T> UrlResource<T> where T: Fetchable {
    pub async fn fetch(&mut self) -> Result<()> {
        match self {
            UrlResource::Resource(_) => {},
            UrlResource::Url(url) => {
                *self = UrlResource::Resource(T::fetch(&url).await?);
            }
        }
        Ok(())
    }
}

#[async_trait]
trait Fetchable where Self: Sized {
    async fn fetch(url: &str) -> Result<Self>;
}

#[derive(Debug)]
pub struct File {
    lines: VecDeque<String>,
}

impl File {
    pub async fn new(url: &str) -> Result<Self> {
        Self::fetch(url).await
    }

    pub fn get_line(&mut self) -> Option<String> {
        self.lines.pop_front()
    }
}

#[async_trait]
impl Fetchable for File {
    async fn fetch(url: &str) -> Result<Self> {
        debug!("Fetching lines from file: {}", url);
        let contents = get_page_contents(&url).await?;
        let raw_url = Document::from(contents.as_str())
                .find(Attr("id","raw-url"))
                .next()
                .map(|x| {
                    x.attr("href").map(|s| format!("https://github.com{}",s))
                })
                .unwrap_or(None).expect("");
        let contents = get_page_contents(&raw_url).await?;
        let lines: VecDeque<_> = contents.split_terminator("\n").map(|s| s.trim()).filter(|s| {
            s.len() >= 10 && s.len() <= 80
            &&
            !s.starts_with("//")
        }).map(|s| {
            s.to_owned()
        }).collect();
        let file = File {
            lines,
        };
        Ok(file)
    }
}

#[derive(Debug)]
pub struct Folder {
    files: VecDeque<UrlResource<File>>,
    folders: VecDeque<UrlResource<Folder>>,
}

impl Folder {
    pub async fn new(url: &str) -> Result<Self> {
        Self::fetch(url).await
    }

    async fn get_file(&mut self) -> Result<Option<File>> {
        if let Some(url) = self.files.pop_front() {
            Ok(Some(match url {
                UrlResource::Resource(file) => {
                    file
                },
                UrlResource::Url(url) => {
                    File::new(&url).await?
                }
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_folder(&mut self) -> Result<Option<Folder>> {
        if let Some(url) = self.folders.pop_front() {
            Ok(Some(match url {
                UrlResource::Resource(folder) => {
                    folder
                },
                UrlResource::Url(url) => {
                    Folder::new(&url).await?
                }
            }))
        } else {
            Ok(None)
        }
    }

    #[async_recursion]
    pub async fn get_line(&mut self) -> Result<Option<String>> {
        while let Some(mut file) = self.get_file().await? {
            if let Some(line) = file.get_line() {
                self.files.push_front(UrlResource::Resource(file));
                return Ok(Some(line));
            }
        }
        while let Some(mut folder) = self.get_folder().await? {
            if let Some(line) = folder.get_line().await? {
                self.folders.push_front(UrlResource::Resource(folder));
                return Ok(Some(line));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl Fetchable for Folder {
    async fn fetch(url: &str) -> Result<Self> {
        // let repo_url = repo_url.string;
        let mut file_urls = VecDeque::new();
        let mut folder_urls = VecDeque::new();

        let contents = get_page_contents(url).await?;
        let document = Document::from(contents.as_str());

        for node in document.find(Attr("role","rowheader").descendant(Name("a"))) {
            match node.attr("rel") {
                Some(_) => (),
                None => {
                    match node.attr("href") {
                        None => (),
                        Some(s) => {
                            if s.contains("blob") && s.ends_with(".rs") {
                                file_urls.push_back(format!("https://github.com{}",s));
                            } else if s.contains("tree") {
                                folder_urls.push_back(format!("https://github.com{}",s));
                            }
                        }
                    }
                },
            }
        }
        let files = file_urls.into_iter().map(|s| UrlResource::Url(s)).collect();
        let folders = folder_urls.into_iter().map(|s| UrlResource::Url(s)).collect();
        Ok(Folder {
            files,
            folders,
        })
        // Err(anyhow::anyhow!("not implemented yet"))
    }
}

// pub type Repo = Folder;
#[derive(Debug)]
pub struct Repo {
    source: String,
    folder: UrlResource<Folder>,
}

impl Repo {
    pub fn new(source: String, url: String) -> Self {
        Repo {
            source,
            folder: UrlResource::Url(url),
        }
    }

    async fn get_line_no_src(&mut self) -> Result<Option<String>> {
        self.folder.fetch().await?;
        if let UrlResource::Resource(ref mut folder) = self.folder {
            folder.get_line().await
        } else {
            Err(anyhow!("folder should be of Resource type"))
        }
    }

    pub async fn get_line(&mut self) -> Result<Option<SrcString>> {
        self.get_line_no_src().await.map(|x| x.map(|s| {
            SrcString {
                source: self.source.clone(),
                string: s,
            }
        }))
    }
}

async fn get_page_contents(url: &str) -> Result<String> {
    trace!("Fetching url: {}", url);
    let client = Client::builder().user_agent(APP_USER_AGENT).build()?;
    let response = client.get(url).send().await?;
    if response.status() != StatusCode::OK {
        bail!("Error Code: {} trying to fetch {}",response.status(),url);
    }
    let text = response.text().await?;
    Ok(text)
}

pub async fn get_repo_urls(page_no: u32) -> Result<VecDeque<Repo>> {
    debug!("Fetching repo urls from page {}", page_no);
    let url = format!("{}&page={}",BASE_CRATES_URL,page_no);
    let json_str = get_page_contents(&url).await?;
    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
    let mut results = VecDeque::new();
    if let serde_json::Value::Array(v) = &json_val["crates"] {
        for crat in v {
            if let serde_json::Value::String(repo_url) = &crat["repository"] {
                if repo_url.contains("github.com") {
                    if let serde_json::Value::String(id) = &crat["id"] {
                        results.push_back(Repo::new(id.to_owned(),repo_url.to_owned()));
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
