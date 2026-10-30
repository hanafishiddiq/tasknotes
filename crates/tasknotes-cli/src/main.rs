use std::io::{BufRead, IsTerminal, Write};

use tasknotes_cli::access::Environment;
use tasknotes_core::audit::Interface;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let env = Environment {
        data_dir: std::env::var_os("TASKNOTES_DATA_DIR"),
        home: std::env::var_os("HOME"),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
        stdin_is_terminal: std::io::stdin().is_terminal(),
    };
    let outcome = tasknotes_cli::run(&args, &env, |interface| {
        // Only reached by `access grant` on an interactive terminal.
        let through = match interface {
            Interface::Mcp => "MCP clients you set up to start tasknotes-mcp",
            _ => "the tasknotes command",
        };
        eprint!(
            "Allow local AI clients to read and change your Tasks and Notes \
             (except Notes marked Exclude from AI) through {through}? \
             Type allow to confirm: "
        );
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        Ok(line)
    });
    let mut stdout = std::io::stdout().lock();
    // Nothing useful can be reported if stdout itself is gone.
    let _ = writeln!(stdout, "{}", outcome.body);
    let _ = stdout.flush();
    std::process::exit(outcome.exit_code);
}
