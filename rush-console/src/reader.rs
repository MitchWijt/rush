use std::io::{stdout, Stdout};
use std::ops::Range;

use anyhow::Result;
use crossterm::cursor::{self, MoveToNextLine};
use crossterm::event::{read, Event, KeyCode, KeyModifiers};
use crossterm::style::{Print, Stylize};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};

use rush_state::shell::Context;

// Represents an action that the handler instructs the REPL (Console.read()) to perform
// Allows for some actions to be performed in the handler and some to be performed in the REPL
enum ReplAction {
    // Instruction to return the line buffer to the shell and perform any necessary cleanup
    Return,
    // Instruction to clear the line buffer and re-prompt the user
    Clear,
    // Instruction to exit the shell
    Exit,
    // Instruction to do nothing
    Ignore,
}

// Allows for reading a line of input from the user through the .read() method
// Handles all the actual terminal interaction between when the method is invoked and
// when the command is actually returned, such as line buffering etc
pub struct Console {
    // * Stdout is stored to prevent repeated std::io::stdout() calls
    stdout: Stdout,
    // A string that stores the current line of input
    // When the user hits ENTER, the line buffer is returned to the shell
    line_buffer: String,
    // The "coordinate" of the cursor is a one-dimensional index of the cursor in the buffer
    cursor_coord: usize,
}

impl Console {
    pub fn new() -> Self {
        Self {
            stdout: stdout(),
            line_buffer: String::new(),
            cursor_coord: 0,
        }
    }

    // TODO: Map crossterm errors to custom errors
    // Prompts the user and handles all input keypresses/resulting terminal interaction up until a line of input is entered
    pub fn read(&mut self, context: &Context) -> Result<String> {
        terminal::enable_raw_mode()?;
        self.print_prompt(context)?;

        loop {
            execute!(self.stdout)?;
            let event = read()?;
            let action = self.handle_event(event)?;

            // self.print_debug_text(1, format!("Raw buffer: {}", self.line_buffer))?;
            // self.print_debug_text(1, format!("Terminal X size: {} | Terminal Y size: {}", terminal::size()?.0, terminal::size()?.1))?;
            // self.print_debug_text(2, format!("Cursor X: {} | Cursor Y: {}", cursor::position()?.0, cursor::position()?.1))?;

            match action {
                ReplAction::Return => {
                    execute!(self.stdout, MoveToNextLine(1))?;
                    terminal::disable_raw_mode()?;
                    let line = self.line_buffer.clone();
                    self.line_buffer.clear();
                    self.cursor_coord = 0;
                    self.clear_debug_text(1..2)?;
                    return Ok(line);
                }
                ReplAction::Clear => {
                    self.line_buffer.clear();
                    self.cursor_coord = 0;
                    self.clear_terminal()?;
                    self.print_prompt(context)?;
                }
                ReplAction::Exit => {
                    self.clear_terminal()?;
                    execute!(self.stdout)?;
                    terminal::disable_raw_mode()?;
                    std::process::exit(0);
                }
                ReplAction::Ignore => (),
            }
        }
    }

    // Handles a key event by queueing appropriate commands based on the given keypress
    // * The bool is essentially a "should return" flag. This will be changed in the future.
    // TODO: Change this return type
    fn handle_event(&mut self, event: Event) -> Result<ReplAction> {
        if let Event::Key(event) = event {
            // TODO: Functionize most of these match arms
            match (event.modifiers, event.code) {
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => self.insert_char(c)?,
                (KeyModifiers::NONE, KeyCode::Backspace) => {
                    if self.cursor_coord != 0 {
                        self.backspace_char()?;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Left) => {
                    if self.cursor_coord != 0 {
                        self.move_cursor_left()?;
                        self.cursor_coord -= 1;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Right) => {
                    if self.cursor_coord != self.line_buffer.len() {
                        self.move_cursor_right()?;
                        self.cursor_coord += 1;
                    }
                }
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    if !self.line_buffer.is_empty() {
                        return Ok(ReplAction::Return);
                    }
                },
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Ok(ReplAction::Exit),
                (KeyModifiers::CONTROL, KeyCode::Char('l')) => return Ok(ReplAction::Clear),
                _ => (),
            }
        }

        // ? Error if not an Event::Key?
        Ok(ReplAction::Ignore)
    }

    // Moves the cursor to the right, wrapping to the next line if necessary
    fn move_cursor_right(&mut self) -> Result<()> {
        let x_size = terminal::size()?.0;
        let x_pos = cursor::position()?.0;

        if x_pos == x_size - 1 {
            queue!(self.stdout, cursor::MoveToNextLine(1))?;
        } else {
            queue!(self.stdout, cursor::MoveRight(1))?;
        }

        Ok(())
    }

    // Moves the cursor to the left, wrapping to the previous line if necessary
    fn move_cursor_left(&mut self) -> Result<()> {
        let x_size = terminal::size()?.0;
        let x_pos = cursor::position()?.0;

        if x_pos == 0 {
            queue!(self.stdout, cursor::MoveToPreviousLine(1), cursor::MoveRight(x_size - 1))?;
        } else {
            queue!(self.stdout, cursor::MoveLeft(1))?;
        }

        Ok(())
    }

    // Inserts a character into the line buffer at the cursor position
    fn insert_char(&mut self, char: char) -> Result<()> {
        // Insert the char and update the buffer after the cursor
        self.line_buffer.insert(self.cursor_coord, char);
        self.print_buffer_section(false)?;
        self.cursor_coord += 1;
        // Move the cursor right so the text does not get overwritten upon the next insertion
        self.move_cursor_right()?;

        Ok(())
    }

    // Removes the character immediately preceding the cursor position from the line buffer
    fn backspace_char(&mut self) -> Result<()> {
        self.cursor_coord -= 1;
        self.line_buffer.remove(self.cursor_coord);
        self.move_cursor_left()?;
        self.print_buffer_section(true)?;

        Ok(())
    }

    // Prints a section of the line buffer starting from the cursor position
    fn print_buffer_section(&mut self, deletion_mode: bool) -> Result<()> {
        // If deleting a character, print a space at the end of the buffer to prevent
        // the last char in the buffer from being duplicated when shifting the line
        // * This is a better solution than first clearing the line after the cursor
        // * because clearing the line incurs a more noticeable flicker
        let deletion_char = match deletion_mode {
            true => " ",
            false => "",
        };

        queue!(
            self.stdout,
            cursor::SavePosition,
            Print(&self.line_buffer[self.cursor_coord..]),
            Print(deletion_char),
            cursor::RestorePosition,
        )?;

        Ok(())
    }

    // Prints debug text to the bottom lines of the terminal
    #[allow(dead_code)]
    fn print_debug_text(&mut self, line: u16, text: String) -> Result<()> {
        queue!(
            self.stdout,
            cursor::SavePosition,
            cursor::MoveTo(0, terminal::size()?.1 - line),
            Clear(ClearType::UntilNewLine),
            Print(text),
            cursor::RestorePosition,
        )?;

        Ok(())
    }

    // Clears the bottom lines of the terminal
    fn clear_debug_text(&mut self, lines: Range<u16>) -> Result<()> {
        for line in lines {
            queue!(
                self.stdout,
                cursor::SavePosition,
                cursor::MoveTo(0, terminal::size()?.1 - line),
                Clear(ClearType::UntilNewLine),
                cursor::RestorePosition,
            )?;
        }

        Ok(())
    }

    // Clears the entire terminal
    fn clear_terminal(&mut self) -> Result<()> {
        queue!(self.stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;
        Ok(())
    }

    // Queues the prompt to be printed
    fn print_prompt(&mut self, context: &Context) -> Result<()> {
        queue!(self.stdout, Print(generate_prompt(context)))?;
        Ok(())
    }
}

// Generates the prompt string used by the REPL
fn generate_prompt(context: &Context) -> String {
    let user = context.env().USER().clone();
    let home = context.env().HOME();
    let truncation = context.shell_config().truncation_factor;
    let cwd = context.env().CWD().collapse(home, truncation);
    let prompt_delimiter = match context.shell_config().multi_line_prompt {
        true => "\r\n",
        false => " ",
    };

    // ? What is the actual name for this?
    let prompt_tick = match context.success() {
        true => "❯".green(),
        false => "❯".red(),
    }
    .bold();

    format!(
        "\r\n{} on {}{}{} ",
        user.dark_blue(),
        cwd.dark_green(),
        prompt_delimiter,
        prompt_tick
    )
}
