use std::fs::File;
use std::path::Path;
use std::io::{self,Read,Write};
use std::time::{SystemTime,Duration};
use std::convert::TryFrom;

use crossterm::event::{EventStream,KeyCode,Event,KeyEvent,DisableMouseCapture,EnableMouseCapture};
use crossterm::terminal::{SetSize,Clear,ClearType,enable_raw_mode,disable_raw_mode};
use crossterm::cursor::{MoveDown,MoveLeft,MoveTo,Hide,Show,MoveToColumn};
use crossterm::{execute,queue,ErrorKind};
use crossterm::style::{ResetColor,SetForegroundColor,SetBackgroundColor,Color,Attribute};

use futures::{future::FutureExt, select, StreamExt};
use futures_timer::Delay;

#[derive(Debug,Copy,Clone,Default)]
struct TypeResult {
    strlen: u32,
    time: u32,
    mistakes: u32,
}

fn print_centered(cols: u16, line: String) -> u16 {
    let offset: u16 = (cols - u16::try_from(line.len()).expect("String length too long for u16"))/2;
    queue!(io::stdout(),MoveToColumn(offset));
    write!(io::stdout(),"{}",line);
    offset
}

async fn can_you_type(line: String,cols: u16) -> Result<TypeResult,ErrorKind> {
    let mut stdout = io::stdout();

    execute!(stdout,Show);
    let mut start = SystemTime::now();
    let mut has_started = false;
    let mut res = TypeResult::default();
    res.strlen = line.len() as u32;
    let mut reader = EventStream::new();

    let offset = print_centered(cols,line.clone());
    queue!(stdout,MoveDown(1),MoveToColumn(offset));
    stdout.flush();
    let chars = line.chars().collect::<Vec<char>>();
    let mut typed = Vec::<char>::new();

    loop {
        let mut event = reader.next().fuse();
        select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        //println!("Event: {:?}\r",event);
                        match event {
                            Event::Key(KeyEvent {code: KeyCode::Esc, ..}) => break,
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

    execute!(stdout,Hide);

    res.time = u32::try_from(start.elapsed().map_err(|e| ErrorKind::ResizingTerminalFailure(format!("Actually a timing failure: {}",e)))?.as_millis()).unwrap_or(0);

    queue!(stdout,MoveDown(1));
    print_centered(cols,format!("t={}ms",res.time));
    queue!(stdout,MoveDown(2))?;
    stdout.flush();

    Ok(res)
}

fn delay(millis: u64) {
    async_std::task::block_on(async {
        let mut fut = Delay::new(Duration::from_millis(millis)).fuse();
        select! {
            _ = fut => ()
        };
    });
}

fn intro() -> Result<u16,ErrorKind> {
    let mut stdout = io::stdout();

    let (mut cols,mut rows) = crossterm::terminal::size()?;

    if cols < 80 {
        cols = 80;
    }
    if rows < 10 {
        rows = 10;
    }

    execute!(stdout,SetSize(cols,rows));

    queue!(stdout,Clear(ClearType::All),Hide,MoveTo(0,0))?;
    //write!(stdout,"--- WELCOME TO RUSTIC TYPSTER ---")?;
    print_centered(cols,"--- WELCOME TO RUSTIC TYPSTER ---".into());
    queue!(stdout,MoveDown(1))?;
    print_centered(cols,"The typing practice game for Rust".into());
    stdout.flush()?;

    delay(1000);
    queue!(stdout,MoveDown(2))?;
    print_centered(cols,"Get ready to type!".into());
    stdout.flush()?;

    delay(500);
    queue!(stdout,MoveDown(2))?;
    print_centered(cols-16,"3".into());
    stdout.flush()?;

    delay(500);
    print_centered(cols,"2".into());
    stdout.flush()?;

    delay(500);
    print_centered(cols+16,"1".into());
    stdout.flush()?;

    delay(500);
    queue!(stdout,MoveDown(3))?;
    stdout.flush()?;

    Ok(cols)
}

fn start(lines: Vec<String>) -> Result<(),ErrorKind>{

    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnableMouseCapture)?;

    let cols = intro()?;

    let mut results = Vec::new();
    for line in lines {
        results.push(async_std::task::block_on(can_you_type(line,cols)));
    }
    let results = results.into_iter().collect::<Result<Vec<TypeResult>,ErrorKind>>()?;

    execute!(stdout, Show);
    execute!(stdout, DisableMouseCapture)?;
    disable_raw_mode()?;

    println!("\rGOODBYE");

    print_results(results);

    Ok(())
}



fn print_results(results: Vec<TypeResult>) {
    let (cum_len, cum_time, cum_mistakes) = results.iter().fold((0,0,0),|(x,y,z), tr| {
        (x+tr.strlen,y+tr.time, z+tr.mistakes)
    });

    let accuracy = 100.0-((cum_mistakes as f32)*100.0/(cum_len as f32));

    let char_per_min = cum_len*60*1000/cum_time;

    println!("You typed {} chars/min at {:.2}% accuracy", char_per_min, accuracy);
}

fn get_lines(filename: String) -> Result<Vec<String>,io::Error> {

    let mut f = File::open(Path::new(&filename))?;

    let mut contents = String::new();
    f.read_to_string(&mut contents)?;

    Ok(contents.split_terminator("\r\n").filter(|s| s.len() >= 10 && s.len() <= 80).map(|s| s.into()).collect())
}

fn main() {

    let filename = "lines.txt".into();
    let lines = get_lines(filename).unwrap_or(Vec::new());

    start(lines).expect("BIG WOOPS");

}
