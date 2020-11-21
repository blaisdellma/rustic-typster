
// use std::cell::RefCell;
// use std::sync::Arc;

use std::collections::VecDeque;

use curl::easy::Easy;

use select::{document::Document,node::Node,predicate::{Attr,Class,Name,Predicate,And}};

#[derive(Debug,Default)]
pub struct Lines {
    file_urls: Vec<String>,
    lines: VecDeque<String>,
    line_buf_len: usize,
}

impl Lines {
    pub fn new(line_buf_len: usize) -> Self {
        let file_urls = fetch_docs_rs();
        Lines {file_urls, line_buf_len, ..Lines::default()}
    }
}

impl Iterator for Lines {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {

        while self.file_urls.len() > 0 && self.lines.len() < self.line_buf_len {
            self.lines.extend(get_file_contents(self.file_urls.pop().unwrap()));
        }
        self.lines.pop_front()
    }
}

fn get_file_contents(url: String) -> Vec<String> {
    let contents = fetch_html(url);
    contents.split_terminator("\n").map(|s| s.trim()).filter(|s| {
        s.len() >= 10 && s.len() <= 80
        &&
        !s.starts_with("//")
    }).map(|s| s.into()).collect()
}

fn get_crates_urls(base: String, docs_rs_html: String) -> Vec<String> {
    Document::from(docs_rs_html.as_str()).find(Attr("class","recent-releases-container").descendant(Attr("class","release"))).map(|node| {
        base.clone() + node.attr("href").unwrap()
    }).collect()
}

fn get_github_url(docs_rs_html: String) -> Option<String> {
    let document = Document::from(docs_rs_html.as_str());
    let heading_node = document.find(Attr("class","pure-menu-heading")).filter(|node| {
        node.text() == "Links"
    }).take(1).collect::<Vec<Node>>();

    if heading_node.len() == 0 {
        return None;
    }

    let parent_node = heading_node[0].parent().unwrap();

    let urls = parent_node.find(Class("pure-menu-link")).filter_map(|node| {
        match node.attr("href").unwrap() {
            x if x.contains("github.com") => Some(x.into()),
            _ => None,
        }
    }).take(1).collect::<Vec<String>>();
    if urls.len() > 0 {
        //let url = urls[0].clone();
        Some(match urls[0].strip_suffix(".git") {
            Some(url) => url.into(),
            None => urls[0].clone(),
        })
    } else {
        None
    }

}

fn get_file_urls(github_html: &String) -> Vec<String> {
    Document::from(github_html.as_str()).find(And(And(Name("a"), Class("link-gray-dark")), Class("js-navigation-open"))).filter_map(|node| {
        match node.attr("href") {
            Some(x) if x.contains("blob/master/src") && x.ends_with(".rs") => Some(x.into()),
            _ => None,
        }
    }).collect()
}

fn get_folder_urls(github_html: &String) -> Vec<String> {
    Document::from(github_html.as_str()).find(And(And(Name("a"), Class("link-gray-dark")), Class("js-navigation-open"))).filter_map(|node| {
        match node.attr("href") {
            Some(x) if x.contains("tree/master/src") => Some(x.into()),
            _ => None,
        }
    }).collect()
}

fn get_file_and_folder_urls(github_url: String) -> (Vec<String>,Vec<String>) {
    match fetch_html(github_url) {
        html if html.len() > 0 => {
            (get_file_urls(&html),get_folder_urls(&html))
        },
        _ => (Vec::new(),Vec::new()),
    }
}

fn get_all_github_file_urls(github_url: String) -> Vec<String> {
    let (mut file_urls, folder_urls) = get_file_and_folder_urls(github_url);
    for url in folder_urls.iter().map(|url| format!("{}{}","https://github.com",url)) {
        file_urls.extend(get_all_github_file_urls(url));
    }

    file_urls
}

pub fn fetch_html(url: String) -> String {

    super::delay(100);

    print!("Fetching from {}",url);

    let mut data_vec = Vec::new();

    let mut easy = Easy::new();
    easy.url(&url).unwrap();

    let mut transfer = easy.transfer();
    transfer.write_function(|data| {
        data_vec.extend_from_slice(data);
        Ok(data.len())
    }).expect("curl error");
    transfer.perform().unwrap();
    drop(transfer);

    println!(" -> Got {} bytes back!\n",data_vec.len());

    String::from_utf8(data_vec).unwrap()
}

pub fn fetch_docs_rs() -> Vec<String>{
    println!("Scrapping docs.rs ...");

    let docs_rs_releases_url = "https://docs.rs/releases".into();
    let docs_rs_base_url = "https://docs.rs".into();

    let docs_rs_html = fetch_html(docs_rs_releases_url);
    let file_urls = get_crates_urls(docs_rs_base_url,docs_rs_html).iter().filter_map(|url| {
        match fetch_html(url.into()) {
            html if html.len() > 0 => {
                let url = get_github_url(html);
                match url {
                    Some(github_url) => {
                        Some(get_all_github_file_urls(format!("{}{}",github_url,"/tree/master/src")))
                    },
                    None => None
                }
            },
            _ => None,
        }
    }).fold(Vec::new(),|mut x,y| {
        x.extend(y);
        x
    }).iter().map(|url| {
        format!("{}{}","https://raw.githubusercontent.com",url.split_terminator("/").filter_map(|s|{
            if s.to_string() == "blob".to_string() {
                None
            } else {
                Some(s.into())
            }
        }).collect::<Vec<String>>().join("/"))
    }).collect();

    file_urls

}

pub fn fetch_docs_rs_serial() -> Vec<String>{
    println!("Scrapping docs.rs ...");

    let mut file_urls = Vec::new();

    let docs_rs_releases_url = "https://docs.rs/releases".into();
    let docs_rs_base_url = "https://docs.rs".into();

    let docs_rs_html = fetch_html(docs_rs_releases_url);
    let crate_urls = get_crates_urls(docs_rs_base_url,docs_rs_html);

    let github_urls: Vec<String> = crate_urls.iter().filter_map(|url| {
        match fetch_html(url.into()) {
            html if html.len() > 0 => {
                get_github_url(html)
            },
            _ => None,
        }
    }).collect();

    for url in &github_urls {
        file_urls.extend(get_all_github_file_urls(format!("{}{}",url,"/tree/master/src")));
    }

    for url in &file_urls {
        println!("{}", url);
    }

    file_urls.iter().map(|url| {
        format!("{}{}","https://raw.githubusercontent.com",url.split_terminator("/").filter_map(|s|{
            if s.to_string() == "blob".to_string() {
                None
            } else {
                Some(s.into())
            }
        }).collect::<Vec<String>>().join("/"))
    }).collect()

}
