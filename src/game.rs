use std::io::{self,Write};
use std::time::{SystemTime,Duration};
use std::convert::TryFrom;
use std::sync::Arc;

use anyhow::Result;

use crossterm::{
    execute,queue,
    event::{EventStream,KeyCode,Event,KeyEvent,DisableMouseCapture,EnableMouseCapture},
    terminal::{SetSize,Clear,ClearType,enable_raw_mode,disable_raw_mode},
    cursor::{MoveLeft,MoveDown,MoveTo,Hide,Show,MoveToColumn},
    style::{ResetColor,SetForegroundColor,SetBackgroundColor,Color,Attribute}
};

use futures::{future::FutureExt, select, StreamExt, try_join};
use futures_timer::Delay;

use crate::line_gen::LineGenerator;

fn print_centered(cols: u16, line: String) -> Result<u16> {
    let line_len = u16::try_from(line.len())?;
    let offset: u16 = (cols - line_len)/2;
    queue!(io::stdout(),Clear(ClearType::CurrentLine),MoveToColumn(offset))?;
    write!(io::stdout(),"{}",line)?;
    Ok(offset)
}

async fn delay(millis: u64) {
    Delay::new(Duration::from_millis(millis)).await
}

async fn show_intro() -> Result<u16> {
    let (mut cols,mut rows) = crossterm::terminal::size()?;
    if cols < 80 {
        cols = 80;
    }
    if rows < 10 {
        rows = 10;
    }
    execute!(io::stdout(),SetSize(cols,rows))?;

    queue!(io::stdout(),Clear(ClearType::All),Hide,MoveTo(0,0))?;
    print_centered(cols,"--- WELCOME TO RUSTIC TYPSTER ---".into())?;
    queue!(io::stdout(),MoveTo(0,1))?;
    print_centered(cols,"The typing practice game for Rust".into())?;
    io::stdout().flush()?;

    delay(1000).await;
    queue!(io::stdout(),MoveTo(0,3))?;
    print_centered(cols,"Get ready to type!".into())?;
    io::stdout().flush()?;

    delay(500).await;
    queue!(io::stdout(),MoveTo(0,6))?;
    print_centered(cols-16,"3".into())?;
    io::stdout().flush()?;

    delay(500).await;
    print_centered(cols,"2".into())?;
    io::stdout().flush()?;

    delay(500).await;
    print_centered(cols+16,"1".into())?;
    io::stdout().flush()?;

    delay(500).await;
    Ok(cols)
}

#[tokio::main]
pub async fn run() -> Result<()>{

    enable_raw_mode()?;
    execute!(io::stdout(), EnableMouseCapture)?;

    defer! {
        match execute!(io::stdout(), Show, DisableMouseCapture) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error in scopeguard: {:#?}",e);
            },
        }
        match disable_raw_mode() {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error in scopeguard: {:#?}",e);
            },
        }
    }

    let (cols,line_gen) = try_join!(show_intro(),LineGenerator::new(10))?;
    let line_gen_mutex = Arc::new(tokio::sync::Mutex::new(line_gen));

    queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::CurrentLine))?;
    io::stdout().flush()?;

    let mut reader = EventStream::new();
    let mut start = SystemTime::now();

    let mut need_line = true;
    let mut has_started = false;
    let mut join_handle = None;

    let mut line: String;
    let mut source: String;
    let mut typed = Vec::<char>::new();
    let mut chars = Vec::<char>::new();
    let mut offset: u16;

    let mut num_mistakes = 0u32;
    let mut num_chars = 0u32;
    let mut num_millis = 0u32;

    loop {
        let mut event = reader.next().fuse();
        let mut flag = false;

        if need_line {
            (line, source) = match line_gen_mutex.try_lock() {
                Ok(mut mutex) => {
                    match mutex.next_line() {
                        Some(src_str) => {
                            need_line = false;
                            queue!(io::stdout(),Show)?;
                            io::stdout().flush()?;
                            (src_str.string, src_str.source)
                        },
                        None => {
                            flag = true;
                            ("Waiting on line ...".into(), "".into())
                        },
                    }
                },
                _ => ("Waiting on line ...".into(), "".into())
            };

            if flag {
                if join_handle.is_none() {
                    let line_gen_mutex2 = line_gen_mutex.clone();
                    join_handle = Some(tokio::task::spawn(async move {
                        line_gen_mutex2.lock().await.extend().await.expect("Extending lines");
                    }).fuse());
                }
            }
            
            queue!(io::stdout(),MoveTo(0,5),Clear(ClearType::FromCursorDown))?;
            print_centered(cols,format!("FROM: {}",source))?;
            queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::FromCursorDown))?;
            offset = print_centered(cols,line.clone())?;
            typed.clear();
            queue!(io::stdout(),MoveTo(0,7),MoveToColumn(offset))?;
            has_started = false;
            chars = line.chars().collect::<Vec<char>>();
            io::stdout().flush()?;
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
                                    queue!(io::stdout(),Hide,MoveTo(0,5))?;
                                    print_centered(cols,format!("t = {}ms",elapsed_time))?;
                                    io::stdout().flush()?;
                                } else if typed.len() == 0 {
                                    need_line = true;
                                }
                            },
                            Event::Key(KeyEvent {code: KeyCode::Backspace, ..}) => {
                                    if typed.len() != 0 {
                                        typed.pop();
                                        queue!(io::stdout(),MoveLeft(1))?;
                                        write!(io::stdout()," ")?;
                                        queue!(io::stdout(),MoveLeft(1))?;
                                        io::stdout().flush()?;
                                    }
                            },
                            Event::Key(KeyEvent {code: KeyCode::Char(x), ..}) => {
                                if !has_started {
                                    has_started = true;
                                    start = SystemTime::now();
                                }
                                typed.push(x);

                                if typed.len() > chars.len() || chars[typed.len()-1] != x {
                                    queue!(io::stdout(),SetForegroundColor(Color::AnsiValue(13)),SetBackgroundColor(Color::AnsiValue(13)))?;
                                    write!(io::stdout(),"{}{}{}",Attribute::Underlined,x,Attribute::NoUnderline)?;
                                    queue!(io::stdout(),ResetColor)?;
                                    num_mistakes += 1;
                                } else {
                                    write!(io::stdout(),"{}",x)?;
                                }
                                io::stdout().flush()?;

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

    if num_chars > 0 {
        let accuracy = 100.0-((num_mistakes as f32)*100.0/(num_chars as f32));
        let char_per_min = num_chars*60*1000/num_millis;
        queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::FromCursorDown))?;
        print_centered(cols,format!("You typed {} chars/min at {:.2}% accuracy", char_per_min, accuracy))?;
        queue!(io::stdout(),MoveDown(1),MoveToColumn(0))?;
        io::stdout().flush()?;
    }

    Ok(())
}
