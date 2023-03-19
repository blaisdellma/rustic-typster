use std::convert::TryFrom;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;

use crossterm::event::{EventStream,KeyCode,Event,KeyEvent};

use futures::{future::FutureExt, select, StreamExt, try_join};

use tokio::sync::Mutex;

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

fn get_next_line(line_gen_mutex: &Arc<Mutex<LineQueue>>) -> Result<Option<SrcString>> {
    let src_str = match line_gen_mutex.try_lock() {
        Ok(mut mutex) => {
            match mutex.next_line() {
                Some(src_str) => {
                    // line found
                    show_cursor()?;
                    Some(src_str)
                },
                None => {
                    // no lines in queue
                    // trigger line_gen to fetch more
                    None
                },
            }
        },
        // mutext not available, wait til next loop
        _ => None
    };
    Ok(src_str)
}

#[tokio::main]
pub async fn run() -> Result<()>{
//     run_it().await
// }
//
// async fn run_it() -> Result<()> {

    let _guard = setup_tui()?;

    let (cols,line_gen) = try_join!(show_intro(),LineQueue::new(10))?;
    let line_gen_mutex = Arc::new(Mutex::new(line_gen));
    clear_countdown()?;

    let mut reader = EventStream::new();
    let mut start = SystemTime::now();

    let mut need_line = true;
    let mut has_started = false;
    let mut line_gen_jh = None;

    let mut src_str: SrcString;
    let mut line: &str = "";
    let mut source: &str;
    let mut typed = Vec::<char>::new();
    let mut chars = Vec::<char>::new();

    let mut stats = TypingStats::new();

    loop {
        let mut event = reader.next().fuse();

        // fetch next line from queue
        if need_line {
            src_str = match get_next_line(&line_gen_mutex)? {
                Some(src_str) => {
                    src_str
                },
                None => {
                    // does line_gen need to fetch more lines
                    // if we're not already waiting on line_gen
                    // spawn task to fill line queue
                    if line_gen_jh.is_none() {
                        let line_gen_mutex2 = line_gen_mutex.clone();
                        line_gen_jh = Some(tokio::task::spawn(async move {
                            line_gen_mutex2.lock().await.extend().await.expect("Extending lines");
                        }).fuse());
                    }
                    SrcString::default()
                }
            };
            need_line = false;

            line = &src_str.string;
            source = &src_str.source;
            
            // show current line with source
            display_current_line(cols,line,source)?;

            // typing setup
            has_started = false;
            chars = line.chars().collect::<Vec<char>>();
            typed.clear();
        }

        line_gen_jh = match line_gen_jh {
            Some(mut jh) => {
                select! {
                    _ = jh => {
                        None
                    },
                    event = event => {
                        match event {
                            Some(Ok(event)) => {
                                match event {
                                    Event::Key(KeyEvent {code: KeyCode::Esc, ..}) => {
                                        break;
                                    },
                                    _ => (),
                                }
                            },
                            Some(Err(e)) => {
                                return Err(e.into());
                            },
                            None => break,
                        }
                        Some(jh)
                    }
                }
            },
            None => {
                match event.await {
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
                None
            }
        }
    }

    show_results(cols, stats)?;

    Ok(())
}
