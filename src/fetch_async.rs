use anyhow::{Result,bail};
use reqwest::{Client,StatusCode};
use select::{document::Document,predicate::{Predicate,Attr,Name}};

const BASE_CRATES_URL: &str = "https://crates.io/api/v1/crates?sort=recent-downloads";

static APP_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

#[derive(Debug,Default)]
struct LineGenerator {
    min_buf_len: usize,
    repo_urls: Vec<String>,
    file_urls: Vec<String>,
    lines: Vec<String>,
    page_no: u32,
}

impl LineGenerator {
    async fn new(min_buf_len: usize) -> Result<Self> {
        Ok((Self {min_buf_len, page_no: 1, ..Self::default()}).init().await?)
    }

    async fn init(mut self) -> Result<Self> {
        self.extend_repos().await?;
        while self.lines.len() < self.min_buf_len {
            self.extend_lines().await?;
        }
        Ok(self)
    }

    async fn next_line(&mut self) -> Result<String> {
        loop {
            match self.lines.pop() {
                Some(s) => return Ok(s),
                None => {
                    self.extend_lines().await?;
                }
            }
        }
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
            let repo_url = self.repo_urls.pop().unwrap();
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
            let file_url = self.file_urls.pop().unwrap();
            to_add = get_lines(file_url).await?;
        }
        self.lines.extend(to_add);
        Ok(self.lines.len())
    }

}

async fn get_page_contents(url: String) -> Result<String> {
    eprint!("Fetching: {} ",url);
    let client = Client::builder().user_agent(APP_USER_AGENT).build()?;
    let response = client.get(&url).send().await?;
    if response.status() != StatusCode::OK {
        bail!("Error Code: {} trying to fetch {}",response.status(),url);
    }
    let text = response.text().await?;
    eprintln!("Done!");
    Ok(text)
}

async fn get_repo_urls(page_no: u32) -> Result<Vec<String>> {
    let url = format!("{}&page={}",BASE_CRATES_URL,page_no);
    let json_str = get_page_contents(url).await?;
    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
    let mut results = Vec::new();
    if let serde_json::Value::Array(v) = &json_val["crates"] {
        for crat in v {
            if let serde_json::Value::String(s) = &crat["repository"] {
                if s.contains("github.com") {
                    results.push(s.into());
                }
            }
        }
    }
    if results.len() == 0 {
        bail!("No crates found!");
    }
    Ok(results)
}

async fn get_file_urls(repo_url: String) -> Result<(Vec<String>,Vec<String>)> {
    let mut file_urls = Vec::new();
    let mut folder_urls = Vec::new();

    eprintln!("Searching in {}",repo_url);
    let contents = get_page_contents(repo_url).await?;
    let document = Document::from(contents.as_str());

    // let og_url = match document.find(And(Name("meta"),Attr("property","og:url"))).next() {
    //     Some(node) => match node.attr("content") {
    //         Some(s) => s,
    //         None => bail!("Could not find og url for {}",repo_url),
    //     },
    //     None => bail!("Could not find og url for {}",repo_url),
    // };

    for node in document.find(Attr("role","rowheader").descendant(Name("a"))) {
        match node.attr("rel") {
            Some(_) => (),
            None => {
                match node.attr("href") {
                    None => (),
                    Some(s) => {
                        if s.contains("blob") && s.ends_with(".rs") {
                            match convert_file_url_to_raw(format!("https://github.com{}",s)).await? {
                                Some(x) => file_urls.push(x),
                                None => (),
                            }
                        } else if s.contains("tree") {
                            folder_urls.push(format!("https://github.com{}",s));
                        }
                    }
                }
            },
        }
    }


    Ok((file_urls,folder_urls))
}

async fn convert_file_url_to_raw(file_url: String) -> Result<Option<String>> {
    let contents = get_page_contents(file_url).await?;
    let document = Document::from(contents.as_str());
    Ok(match document.find(Attr("id","raw-url")).next() {
        None => None,
        Some(x) => match x.attr("href") {
            None => None,
            Some(x) => Some(format!("https://github.com{}",x)),
        },
    })
}

async fn get_lines(file_url: String) -> Result<Vec<String>> {
    let contents = get_page_contents(file_url).await?;
    let lines = contents.split_terminator("\n").map(|s| s.trim()).filter(|s| {
        s.len() >= 10 && s.len() <= 80
        &&
        !s.starts_with("//")
    }).map(|s| s.into()).collect();

    Ok(lines)
}

#[tokio::main]
async fn main_rustic_typster() -> Result<()> {
    let mut line_gen = LineGenerator::new(10).await?;
    // println!("Print repo urls:");
    // for s in line_gen.repo_urls {
    //     println!("{}",s);
    // }
    // println!("Print file urls:");
    // for s in line_gen.file_urls {
    //     println!("{}",s);
    // }
    // println!("Lines:");
    // for s in line_gen.lines {
    //     println!("{}",s);
    // }
    for _ in 0..100 {
        println!("{}",line_gen.next_line().await?);
    }
    Ok(())
}

pub fn start_rustic_typster() {
    println!("Running Rustic Typster");
    main_rustic_typster().unwrap();
    println!("Done");
}
