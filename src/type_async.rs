use std::io::{self,Write};
use std::time::{SystemTime,Duration};
use std::convert::TryFrom;
use std::sync::Arc;

use anyhow::Result;

use tokio::sync::Mutex;

use crossterm::{
    execute,queue,
    event::{EventStream,KeyCode,Event,KeyEvent,DisableMouseCapture,EnableMouseCapture},
    terminal::{SetSize,Clear,ClearType,enable_raw_mode,disable_raw_mode},
    cursor::{MoveLeft,MoveTo,Hide,Show,MoveToColumn},
    style::{ResetColor,SetForegroundColor,SetBackgroundColor,Color,Attribute}
};

use futures::{future::FutureExt, select, StreamExt, try_join};
use futures_timer::Delay;

use crate::fetch_async::LineGenerator;

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
    let mut stdout = io::stdout();

    let (mut cols,mut rows) = crossterm::terminal::size()?;

    if cols < 80 {
        cols = 80;
    }
    if rows < 10 {
        rows = 10;
    }

    execute!(stdout,SetSize(cols,rows))?;

    queue!(stdout,Clear(ClearType::All),Hide,MoveTo(0,0))?;
    //write!(stdout,"--- WELCOME TO RUSTIC TYPSTER ---")?;
    print_centered(cols,"--- WELCOME TO RUSTIC TYPSTER ---".into())?;
    queue!(stdout,MoveTo(0,1))?;
    print_centered(cols,"The typing practice game for Rust".into())?;
    stdout.flush()?;

    delay(1000).await;
    queue!(stdout,MoveTo(0,3))?;
    print_centered(cols,"Get ready to type!".into())?;
    stdout.flush()?;

    delay(500).await;
    queue!(stdout,MoveTo(0,6))?;
    print_centered(cols-16,"3".into())?;
    stdout.flush()?;

    delay(500).await;
    print_centered(cols,"2".into())?;
    stdout.flush()?;

    delay(500).await;
    print_centered(cols+16,"1".into())?;
    stdout.flush()?;

    delay(500).await;

    Ok(cols)
}

#[tokio::main]
pub async fn main_type_async() -> Result<()>{

    enable_raw_mode()?;
    execute!(io::stdout(), EnableMouseCapture)?;

    let intro = show_intro();
    let line_gen = LineGenerator::new(10);

    let (cols,line_gen) = try_join!(intro,line_gen)?;
    let line_gen_mutex = Arc::new(Mutex::new(line_gen));

    queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::CurrentLine))?;
    io::stdout().flush()?;

    let mut reader = EventStream::new();

    let mut need_line = true;
    let mut has_started = false;

    let mut join_handle = None;

    let mut typed = Vec::<char>::new();

    let mut line: String;

    let mut offset: u16;

    let mut start = SystemTime::now();

    let mut chars = Vec::<char>::new();

    let mut num_mistakes = 0u32;
    let mut num_chars = 0u32;
    let mut num_millis = 0u32;

    loop {
        let mut event = reader.next().fuse();
        let mut flag = false;
        if need_line {
            line = match line_gen_mutex.try_lock() {
                Ok(mut mutex) => {
                    match mutex.next_line() {
                        Some(x) => {
                            need_line = false;
                            queue!(io::stdout(),Show)?;
                            io::stdout().flush()?;
                            x
                        },
                        None => {
                            flag = true;
                            "Waiting on line ...".into()
                        },
                    }
                },
                _ => "Waiting on line ...".into(),
            };
            if flag {
                if join_handle.is_none() {
                    let line_gen_mutex2 = line_gen_mutex.clone();
                    join_handle = Some(tokio::task::spawn(async move {
                        line_gen_mutex2.lock().await.extend().await.expect("Extending lines");
                    }).fuse());
                }
            }
            queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::FromCursorDown))?;
            offset = print_centered(cols,line.clone())?;
            typed.clear();
            queue!(io::stdout(),MoveTo(offset,7))?;
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
                                execute!(io::stdout(), Show)?;
                                execute!(io::stdout(), DisableMouseCapture)?;
                                disable_raw_mode()?;
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
                                    queue!(io::stdout(),Hide)?;
                                    io::stdout().flush()?;
                                }
                            },
                            Event::Key(KeyEvent {code: KeyCode::Backspace, ..}) => {
                                    if typed.len() != 0 {
                                        typed.pop();
                                        let mut stdout = io::stdout();
                                        queue!(stdout,MoveLeft(1))?;
                                        write!(stdout," ")?;
                                        queue!(stdout,MoveLeft(1))?;
                                        stdout.flush()?;
                                    }
                            },
                            Event::Key(KeyEvent {code: KeyCode::Char(x), ..}) => {
                                if !has_started {
                                    has_started = true;
                                    start = SystemTime::now();
                                }
                                typed.push(x);
                                let mut stdout = io::stdout();
                                if typed.len() > chars.len() || chars[typed.len()-1] != x {
                                    queue!(stdout,SetForegroundColor(Color::AnsiValue(13)),SetBackgroundColor(Color::AnsiValue(13)))?;
                                    write!(stdout,"{}{}{}",Attribute::Underlined,x,Attribute::NoUnderline)?;
                                    queue!(stdout,ResetColor)?;
                                    num_mistakes += 1;
                                } else {
                                    write!(stdout,"{}",x)?;
                                }
                                stdout.flush()?;

                            },
                            _ => (),
                        }
                    },
                    Some(Err(e)) => {
                        execute!(io::stdout(), Show)?;
                        execute!(io::stdout(), DisableMouseCapture)?;
                        disable_raw_mode()?;
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
        io::stdout().flush()?;
    }

    execute!(io::stdout(), Show)?;
    execute!(io::stdout(), DisableMouseCapture)?;
    disable_raw_mode()?;

    Ok(())
}
