use std::convert::TryFrom;
use std::io::{self,Write};
use std::time::Duration;

use anyhow::Result;

use crossterm::{
    execute,queue,
    event::{DisableMouseCapture,EnableMouseCapture},
    terminal::{SetSize,Clear,ClearType,enable_raw_mode,disable_raw_mode},
    cursor::{MoveLeft,MoveDown,MoveTo,Hide,Show,MoveToColumn},
    style::{ResetColor,SetForegroundColor,SetBackgroundColor,Color,Attribute}
};

use futures_timer::Delay;

use scopeguard::{guard,ScopeGuard};

use crate::game::TypingStats;

pub fn setup_tui() -> Result<ScopeGuard<(),impl FnOnce(()) -> ()>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnableMouseCapture)?;

    let guard = guard((), |_| {
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
    });
    Ok(guard)
}

async fn delay(millis: u64) {
    Delay::new(Duration::from_millis(millis)).await
}

pub fn print_centered(cols: u16, line: &str) -> Result<u16> {
    let line_len = u16::try_from(line.len())?;
    let offset: u16 = (cols - line_len)/2;
    queue!(io::stdout(),Clear(ClearType::CurrentLine),MoveToColumn(offset))?;
    write!(io::stdout(),"{}",line)?;
    Ok(offset)
}

pub async fn show_intro() -> Result<u16> {
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

pub fn clear_countdown() -> Result<()> {
    queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::CurrentLine))?;
    io::stdout().flush()?;
    Ok(())
}

pub fn display_current_line(cols: u16, line: &str, source: &str) -> Result<()> {
    queue!(io::stdout(),MoveTo(0,5),Clear(ClearType::FromCursorDown))?;
    print_centered(cols,&format!("FROM: {}",source))?;
    queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::FromCursorDown))?;
    let offset = print_centered(cols,line)?;
    queue!(io::stdout(),MoveTo(0,7),MoveToColumn(offset))?;
    io::stdout().flush()?;
    Ok(())
}

pub fn show_cursor() -> Result<()> {
    queue!(io::stdout(),Show)?;
    io::stdout().flush()?;
    Ok(())
}

// pub fn hide_cursor() -> Result<()> {
//     queue!(io::stdout(),Hide)?;
//     io::stdout().flush()?;
//     Ok(())
// }

pub fn backspace() -> Result<()> {
    queue!(io::stdout(),MoveLeft(1))?;
    write!(io::stdout()," ")?;
    queue!(io::stdout(),MoveLeft(1))?;
    io::stdout().flush()?;
    Ok(())
}

pub fn type_char(c: char, correct: bool) -> Result<()> {
    if correct {
        write!(io::stdout(),"{}",c)?;
    } else {
        queue!(io::stdout(),SetForegroundColor(Color::AnsiValue(13)),SetBackgroundColor(Color::AnsiValue(13)))?;
        write!(io::stdout(),"{}{}{}",Attribute::Underlined,c,Attribute::NoUnderline)?;
        queue!(io::stdout(),ResetColor)?;
    }
    io::stdout().flush()?;
    Ok(())
}

pub fn show_time(cols: u16, elapsed_time: u32) -> Result<()> {
    queue!(io::stdout(),Hide,MoveTo(0,5))?;
    print_centered(cols,&format!("t = {}ms",elapsed_time))?;
    io::stdout().flush()?;
    Ok(())
}

pub fn show_results(cols: u16, stats: TypingStats) -> Result<()> {
    if stats.total_chars > 0 {
        let accuracy = 100.0-((stats.total_mistakes as f32)*100.0/(stats.total_chars as f32));
        let char_per_min = stats.total_chars*60*1000/stats.total_time_ms;
        queue!(io::stdout(),MoveTo(0,6),Clear(ClearType::FromCursorDown))?;
        print_centered(cols,&format!("You typed {} chars/min at {:.2}% accuracy", char_per_min, accuracy))?;
        queue!(io::stdout(),MoveDown(1),MoveToColumn(0))?;
        io::stdout().flush()?;
    }
    Ok(())
}
