use std::convert::TryFrom;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;

use crossterm::event::{EventStream,KeyCode,Event,KeyEvent};

use futures::{future::FutureExt, select, StreamExt, try_join};

use crate::line_gen::*;
use crate::tui::*;

#[tokio::main]
pub async fn run() -> Result<()>{

    let _guard = setup_tui()?;

    let (cols,line_gen) = try_join!(show_intro(),LineGenerator::new(10))?;
    let line_gen_mutex = Arc::new(tokio::sync::Mutex::new(line_gen));
    clear_countdown()?;

    let mut reader = EventStream::new();
    let mut start = SystemTime::now();

    let mut need_line = true;
    let mut has_started = false;
    let mut join_handle = None;

    let mut line: String;
    let mut source: String;
    let mut typed = Vec::<char>::new();
    let mut chars = Vec::<char>::new();

    let mut num_mistakes = 0u32;
    let mut num_chars = 0u32;
    let mut num_millis = 0u32;

    loop {
        let mut event = reader.next().fuse();
        let mut need_extend = false;

        // fetch next line from queue
        if need_line {
            (line, source) = match line_gen_mutex.try_lock() {
                Ok(mut mutex) => {
                    match mutex.next_line() {
                        Some(src_str) => {
                            // line found
                            need_line = false;
                            show_cursor()?;
                            (src_str.string, src_str.source)
                        },
                        None => {
                            // no lines in queue
                            // trigger line_gen to fetch more
                            need_extend = true;
                            ("Waiting on line ...".into(), "".into())
                        },
                    }
                },
                // mutext not available, wait til next loop
                _ => ("Waiting on line ...".into(), "".into())
            };

            // does line_gen need to fetch more lines
            // if we're not already waiting on line_gen
            // spawn task to fill line queue
            if need_extend {
                if join_handle.is_none() {
                    let line_gen_mutex2 = line_gen_mutex.clone();
                    join_handle = Some(tokio::task::spawn(async move {
                        line_gen_mutex2.lock().await.extend().await.expect("Extending lines");
                    }).fuse());
                }
            }
            
            // show current line with source
            display_current_line(cols,&line,&source)?;

            // typing setup
            has_started = false;
            chars = line.chars().collect::<Vec<char>>();
            typed.clear();
        }

        join_handle = match join_handle {
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
                                    let elapsed_time = u32::try_from(start.elapsed()?.as_millis())?;
                                    num_millis += elapsed_time;
                                    num_chars += typed.len() as u32;
                                    need_line = true;
                                    show_time(cols,elapsed_time)?;
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
                                    num_mistakes += 1;
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

    show_results(cols, num_chars, num_millis, num_mistakes)?;

    Ok(())
}
