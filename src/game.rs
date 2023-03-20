use std::convert::TryFrom;
use std::time::SystemTime;

use anyhow::{bail,Result};

use crossterm::event::{EventStream,KeyCode,Event,KeyEvent};

use futures::StreamExt;

use tokio::{select,sync::mpsc::{channel,Receiver}};

use crate::line_queue::*;
use crate::tui::*;

#[derive(Default)]
pub struct TypingStats {
    pub total_mistakes: u32,
    pub total_chars: u32,
    pub total_time_ms: u32,
}

impl TypingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_line(&mut self, line: &str, time_elapsed_ms: u32) {
        self.total_chars += line.len() as u32;
        self.total_time_ms += time_elapsed_ms;
    }

    pub fn add_mistake(&mut self) {
        self.total_mistakes += 1;
    }
}

async fn get_next_line(rx: &mut Receiver<SrcString>, reader: &mut EventStream) -> Result<Option<SrcString>> {
    loop {
        break select! {
            src_str_opt = rx.recv() => {
                match src_str_opt {
                    Some(x) => Ok(Some(x)),
                    None => bail!("channel is closed"),
                }
            },
            event = reader.next() => {
                match event {
                    Some(Ok(event)) => {
                        match event {
                            Event::Key(KeyEvent {code: KeyCode::Esc, ..}) => {
                                break Ok(None);
                            },
                            _ => continue,
                        }
                    },
                    Some(Err(e)) => {
                        return Err(e.into());
                    },
                    None => break Ok(None),
                }
            }
        };
    }
}

#[tokio::main]
pub async fn run() -> Result<()>{

    let _guard = setup_tui()?;

    let (tx,mut rx) = channel::<SrcString>(10);

    let line_queue = LineQueue::new(tx.clone());

    let cols = show_intro().await?;
    clear_countdown()?;

    let mut reader = EventStream::new();
    let mut start = SystemTime::now();

    let mut need_line = true;
    let mut has_started = false;

    let mut src_str: SrcString;
    let mut line: &str = "";
    let mut source: &str;
    let mut typed = Vec::<char>::new();
    let mut chars = Vec::<char>::new();

    let mut stats = TypingStats::new();

    loop {
        // fetch next line from queue
        if need_line {
            src_str = match get_next_line(&mut rx, &mut reader).await? {
                Some(x) => x,
                None => break,
            };
            need_line = false;

            line = &src_str.string;
            source = &src_str.source;
            
            // show current line with source
            display_current_line(cols,line,source)?;
            show_cursor()?;

            // typing setup
            has_started = false;
            chars = line.chars().collect::<Vec<char>>();
            typed.clear();
        }

        match reader.next().await {
            Some(Ok(event)) => {
                match event {
                    Event::Key(KeyEvent {code: KeyCode::Esc, ..}) => {
                        break;
                    },
                    Event::Key(KeyEvent {code: KeyCode::Enter, ..}) => {
                        if typed == chars {
                            let elapsed_time_ms = u32::try_from(start.elapsed()?.as_millis())?;
                            stats.add_line(line,elapsed_time_ms);
                            need_line = true;
                            show_time(cols,elapsed_time_ms)?;
                        } else if typed.len() == 0 {
                            need_line = true;
                        }
                    },
                    Event::Key(KeyEvent {code: KeyCode::Backspace, ..}) => {
                        if typed.len() != 0 {
                            typed.pop();
                            backspace()?;
                        }
                    },
                    Event::Key(KeyEvent {code: KeyCode::Char(x), ..}) => {
                        if !has_started {
                            has_started = true;
                            start = SystemTime::now();
                        }
                        typed.push(x);

                        if typed.len() > chars.len() || chars[typed.len()-1] != x {
                            type_char(x,false)?;
                            stats.add_mistake();
                        } else {
                            type_char(x,true)?;
                        }

                    },
                    _ => (),
                }
            },
            Some(Err(e)) => {
                return Err(e.into());
            },
            None => break,
        }
    }

    rx.close();
    line_queue.await??;

    show_results(cols, stats)?;

    Ok(())
}
