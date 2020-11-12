use std::fs::File;
use std::path::Path;
use std::io::{self,Read,Write};
use std::time::{SystemTime,Duration};
use std::convert::TryFrom;

use anyhow::Result;

pub mod fetch;

use crossterm::{
    execute,queue,
    event::{EventStream,KeyCode,Event,KeyEvent,DisableMouseCapture,EnableMouseCapture},
    terminal::{SetSize,Clear,ClearType,enable_raw_mode,disable_raw_mode},
    cursor::{MoveDown,MoveLeft,MoveTo,Hide,Show,MoveToColumn},
    style::{ResetColor,SetForegroundColor,SetBackgroundColor,Color,Attribute}
};

use futures::{future::FutureExt, select, StreamExt};
use futures_timer::Delay;

#[derive(Debug,Copy,Clone,Default)]
struct TypeResult {
    strlen: u32,
    time: u32,
    mistakes: u32,
}

fn print_centered(cols: u16, line: String) -> Result<u16> {
    let line_len = u16::try_from(line.len())?;
    let offset: u16 = (cols - line_len)/2;
    queue!(io::stdout(),MoveToColumn(offset))?;
    write!(io::stdout(),"{}",line)?;
    Ok(offset)
}

async fn can_you_type(line: String,cols: u16) -> Result<Option<TypeResult>> {
    let mut stdout = io::stdout();

    let mut start = SystemTime::now();
    let mut has_started = false;
    let mut res = TypeResult {strlen: (line.len() as u32), ..TypeResult::default()};

    let offset = print_centered(cols,line.clone())?;
    queue!(stdout,MoveDown(1),MoveToColumn(offset),Show)?;
    stdout.flush()?;

    let chars = line.chars().collect::<Vec<char>>();
    let mut typed = Vec::<char>::new();

    let mut reader = EventStream::new();

    loop {
        let mut event = reader.next().fuse();
        select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        match event {
                            Event::Key(KeyEvent {code: KeyCode::Esc, ..}) => {
                                return Ok(None);
                            },
                            Event::Key(KeyEvent {code: KeyCode::Enter, ..}) => {
                                if typed == chars {
                                    break;
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
                                    res.mistakes += 1;
                                } else {
                                    write!(stdout,"{}",x)?;
                                }
                                stdout.flush()?;

                            },
                            _ => (),
                        }
                    },
                    None => break,
                    _ => (),
                }
            }
        };
    }

    execute!(stdout,Hide)?;

    res.time = u32::try_from(start.elapsed()?.as_millis())?;

    Ok(Some(res))
}

pub fn delay(millis: u64) {
    async_std::task::block_on(Delay::new(Duration::from_millis(millis)));
}

fn intro() -> Result<u16> {
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

    delay(1000);
    queue!(stdout,MoveTo(0,3))?;
    print_centered(cols,"Get ready to type!".into())?;
    stdout.flush()?;

    delay(500);
    queue!(stdout,MoveTo(0,6))?;
    print_centered(cols-16,"3".into())?;
    stdout.flush()?;

    delay(500);
    print_centered(cols,"2".into())?;
    stdout.flush()?;

    delay(500);
    print_centered(cols+16,"1".into())?;
    stdout.flush()?;

    delay(500);

    Ok(cols)
}

pub fn start(lines: Vec<String>) -> Result<()>{

    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnableMouseCapture)?;

    let cols = intro()?;

    let mut results = Vec::new();
    for line in lines {
        queue!(stdout,MoveTo(0,6),Clear(ClearType::FromCursorDown))?;
        stdout.flush()?;
        match async_std::task::block_on(can_you_type(line,cols)) {
            Ok(Some(res)) => {
                results.push(res);

                queue!(stdout,MoveTo(0,5))?;
                print_centered(cols,format!("t={}ms",res.time))?;
                stdout.flush()?;
            }
            Ok(None) => break,
            Err(e) => return Err(e),
        }
    }

    queue!(stdout,MoveTo(0,5),Clear(ClearType::FromCursorDown))?;
    if !results.is_empty() {
        print_results(cols,results)?;
        queue!(stdout,MoveDown(1))?;
    }

    print_centered(cols,"GOODBYE".into())?;
    stdout.flush()?;

    execute!(stdout, Show)?;
    execute!(stdout, DisableMouseCapture)?;
    disable_raw_mode()?;

    Ok(())
}

fn print_results(cols: u16, results: Vec<TypeResult>) -> Result<()> {

    let (cum_len, cum_time, cum_mistakes) = results.iter().fold((0,0,0),|(x,y,z), tr| {
        (x+tr.strlen,y+tr.time, z+tr.mistakes)
    });

    let accuracy = 100.0-((cum_mistakes as f32)*100.0/(cum_len as f32));
    let char_per_min = cum_len*60*1000/cum_time;

    print_centered(cols,format!("You typed {} chars/min at {:.2}% accuracy", char_per_min, accuracy))?;

    Ok(())
}

pub fn get_lines(filename: String) -> Result<Vec<String>> {

    let mut f = File::open(Path::new(&filename))?;

    let mut contents = String::new();
    f.read_to_string(&mut contents)?;

    Ok(contents.split_terminator("\r\n").filter(|s| s.len() >= 10 && s.len() <= 80).map(|s| s.into()).collect())
}
