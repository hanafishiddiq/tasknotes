use std::io::{BufRead, IsTerminal, Write};

use tasknotes_cli::access::Environment;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let env = Environment {
        data_dir: std::env::var_os("TASKNOTES_DATA_DIR"),
        home: std::env::var_os("HOME"),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
        stdin_is_terminal: std::io::stdin().is_terminal(),
    };
    let outcome = tasknotes_cli::run(&args, &env, |prompt| {
        // Only reached on an interactive terminal (`access grant`, `backup restore`).
        eprint!("{prompt}");
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
