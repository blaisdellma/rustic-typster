use std::fs::File;
use std::path::Path;
use std::io::{self,Read,Write};
use std::time::SystemTime;

use crossterm::event::{EventStream,KeyCode,Event,KeyEvent,DisableMouseCapture,EnableMouseCapture};
use crossterm::terminal::{Clear,ClearType,enable_raw_mode,disable_raw_mode};
use crossterm::cursor::{MoveDown,MoveLeft,MoveTo};
use crossterm::{execute,queue,ErrorKind};
use crossterm::style::{ResetColor,SetForegroundColor,SetBackgroundColor,Color,Attribute};
use futures::{future::FutureExt, select, StreamExt};

#[derive(Debug,Copy,Clone,Default)]
struct TypeResult {
    strlen: u32,
    time: u128,
    mistakes: u32,
}

async fn can_you_type(line: &str) -> Result<TypeResult,ErrorKind> {
    let mut start = SystemTime::now();
    let mut hasStarted = false;
    let mut res = TypeResult::default();
    res.strlen = line.len() as u32;
    let mut reader = EventStream::new();
    println!("{}\r",line);
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
                                    execute!(io::stdout(),MoveDown(1))?;
                                    println!("\r\nYOU DID IT!");
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
                                if !hasStarted {
                                    hasStarted = true;
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

    res.time = start.elapsed().map_err(|e| ErrorKind::ResizingTerminalFailure(format!("Actually a timing failure: {}",e)))?.as_millis();
    Ok(res)
}

fn main() {
    println!("WELCOME TO RUSTIC TYPSTER");

    let filename = "lines.txt";

    let mut f = File::open(Path::new(filename)).expect("File not found.");

    let mut contents = String::new();
    f.read_to_string(&mut contents).expect(&format!("Failed to read contents of file: {}",filename));
    //println!("Contents of file:\n{}",contents);

    let lines: Vec<&str> = contents.split_terminator("\r\n").filter(|s| s.len() >= 10 && s.len() <= 80).collect();

    //println!("Trimmed contents of file:\n{:?}",lines);

    // let mut buf = String::new();
    // let stdin =  io::stdin();
    //
    // print!("Give me some input: ");
    // io::stdout().flush().expect("Error in flushing stdout.");
    // stdin.read_line(&mut buf).expect("Error reading from stdin.");
    // println!("You typed: {}",buf);


    enable_raw_mode().expect("Error in enabling raw mode.");
    execute!(io::stdout(), EnableMouseCapture).expect("Failed enabling mouse capture");
    //println!("Waiting for events:\r");
    //async_std::task::block_on(test_crossterm_event_stream());

    let mut stdout = io::stdout();

    //queue!(stdout,Clear(ClearType::All),SetForegroundColor(Color::AnsiValue(13)),MoveTo(10,10)).expect("Failed setting color red");
    write!(stdout,"Can you type this:\r\n").expect("writing output");
    //queue!(stdout,MoveTo(10,11),ResetColor).expect("Failed resetting color.");
    stdout.flush().expect("Tried to flush.");

    let mut results = Vec::new();

    for line in lines {
        results.push(async_std::task::block_on(can_you_type(line)));
    }

    let results = results.into_iter().collect::<Result<Vec<TypeResult>,ErrorKind>>().expect("Error with results.");

    execute!(io::stdout(), DisableMouseCapture).expect("Failed disabling mouse capture");
    disable_raw_mode().expect("Error in disabling raw mode.");

    println!("\rGOODBYE");

    println!("{:?}",results);

}
