mod builtins;
mod commands;
mod path;
mod shell;

use shell::Shell;

fn main() {
    let mut shell = Shell::new();
    shell.run();
}
