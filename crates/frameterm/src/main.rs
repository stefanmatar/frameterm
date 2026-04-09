mod client;
mod daemon;
mod protocol;

use clap::{Parser, Subcommand};

use crate::protocol::{Request, Response};

#[derive(Parser)]
#[command(
    name = "frameterm",
    about = "Terminal automation framework for AI agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Spawn a new terminal session with a command
    Spawn {
        /// Session name
        #[arg(long)]
        name: Option<String>,

        /// Working directory
        #[arg(long)]
        cwd: Option<String>,

        /// Frames per second for recording (default: 10)
        #[arg(long)]
        fps: Option<u32>,

        /// Disable recording for this session
        #[arg(long)]
        no_record: bool,

        /// Terminal columns
        #[arg(long, default_value_t = 80)]
        cols: u16,

        /// Terminal rows
        #[arg(long, default_value_t = 24)]
        rows: u16,

        /// Command to run
        command: String,

        /// Command arguments
        args: Vec<String>,
    },

    /// Capture the current terminal screen as a snapshot
    Snapshot {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Output format: json (structured), compact (no text), text (plain text, pipeable)
        #[arg(long, default_value = "json")]
        format: String,

        /// Block until content hash changes from this value
        #[arg(long)]
        await_change: Option<String>,

        /// Settle time in ms after change detected
        #[arg(long)]
        settle: Option<u64>,

        /// Timeout in ms (default: 30000)
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Type a string of text into a session
    Type {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Text to type
        text: String,
    },

    /// Send key(s) to a session (e.g. Enter, Ctrl+C, "Escape : w q Enter")
    Key {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Delay between keys in ms (for sequences)
        #[arg(long)]
        delay: Option<u64>,

        /// Key name or space-separated sequence
        keys: String,
    },

    /// Send a mouse click at a terminal position (row, col)
    Click {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Row (0-indexed)
        row: u16,

        /// Column (0-indexed)
        col: u16,
    },

    /// Scroll up or down in a session
    Scroll {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Direction: up or down
        direction: String,

        /// Number of lines to scroll
        #[arg(default_value_t = 3)]
        lines: u16,
    },

    /// Resize a session's terminal dimensions
    Resize {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Columns
        cols: u16,

        /// Rows
        rows: u16,
    },

    /// Block until text or a regex pattern appears on screen
    WaitFor {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Use regex matching
        #[arg(long)]
        regex: bool,

        /// Wait for text to NOT be visible
        #[arg(long)]
        not: bool,

        /// Timeout in ms (default: 30000)
        #[arg(long)]
        timeout: Option<u64>,

        /// Text or pattern to wait for
        pattern: String,
    },

    /// Export session recordings as MP4 video
    #[command(name = "record")]
    Record {
        #[command(subcommand)]
        action: RecordAction,
    },

    /// List all active sessions
    ListSessions,

    /// Kill a session and its child process
    Kill {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,
    },

    /// Stop the daemon and terminate all sessions
    Stop,

    /// Stream JSON commands via stdin/stdout over a single daemon connection
    Pipe,

    /// Start the daemon in the foreground (used internally)
    Daemon,

    /// Print workflow examples and usage patterns
    Examples,
}

#[derive(Subcommand)]
enum RecordAction {
    /// Export session recording to MP4 with optional input overlay
    Export {
        /// Session name
        #[arg(short, long)]
        s: Option<String>,

        /// Export all sessions
        #[arg(long)]
        all: bool,

        /// Disable input overlay (keystrokes, clicks, scrolls)
        #[arg(long)]
        no_overlay: bool,

        /// Remove footer entirely — terminal only
        #[arg(long)]
        no_footer: bool,

        /// Output directory
        #[arg(long)]
        output: Option<String>,

        /// Video width in pixels (default: natural terminal width)
        #[arg(long)]
        width: Option<u32>,
    },
}

fn resolve_session(name: Option<String>) -> String {
    if let Some(n) = name {
        return n;
    }
    if let Ok(env_name) = std::env::var("FRAMETERM_SESSION") {
        if !env_name.is_empty() {
            return env_name;
        }
    }
    "default".to_string()
}

fn main() {
    let cli = Cli::parse();

    if let Commands::Daemon = &cli.command {
        daemon::run_daemon();
        return;
    }

    if let Commands::Examples = &cli.command {
        print_examples();
        return;
    }

    if let Commands::Pipe = &cli.command {
        if let Err(e) = client::ensure_daemon() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        client::run_pipe();
    }

    if let Err(e) = client::ensure_daemon() {
        eprintln!("{e}");
        std::process::exit(1);
    }

    let mut raw_text_output = false;

    let request = match cli.command {
        Commands::Spawn {
            name,
            cwd,
            command,
            args,
            fps,
            no_record,
            cols,
            rows,
        } => Request::Spawn {
            name,
            cmd: command,
            args,
            cwd,
            cols: Some(cols),
            rows: Some(rows),
            fps,
            no_record,
        },

        Commands::Snapshot {
            s,
            format,
            await_change,
            settle,
            timeout,
        } => {
            if format == "text" {
                raw_text_output = true;
            }
            Request::Snapshot {
                session: resolve_session(s),
                format,
                await_change,
                settle,
                timeout,
            }
        }

        Commands::Type { s, text } => Request::Type {
            session: resolve_session(s),
            text,
        },

        Commands::Key { s, keys, delay } => Request::Key {
            session: resolve_session(s),
            keys,
            delay,
        },

        Commands::Click { s, row, col } => Request::Click {
            session: resolve_session(s),
            row,
            col,
        },

        Commands::Scroll {
            s,
            direction,
            lines,
        } => Request::Scroll {
            session: resolve_session(s),
            direction,
            lines,
        },

        Commands::Resize { s, cols, rows } => Request::Resize {
            session: resolve_session(s),
            cols,
            rows,
        },

        Commands::WaitFor {
            s,
            pattern,
            regex,
            not,
            timeout,
        } => Request::WaitFor {
            session: resolve_session(s),
            pattern,
            regex,
            not,
            timeout,
        },

        Commands::Record {
            action:
                RecordAction::Export {
                    s,
                    all,
                    no_overlay,
                    no_footer,
                    output,
                    width,
                },
        } => {
            if all {
                Request::RecordExport {
                    session: None,
                    all: true,
                    no_overlay: no_overlay || no_footer,
                    no_footer,
                    output,
                    width,
                }
            } else {
                Request::RecordExport {
                    session: Some(resolve_session(s)),
                    all: false,
                    no_overlay: no_overlay || no_footer,
                    no_footer,
                    output,
                    width,
                }
            }
        }

        Commands::ListSessions => Request::ListSessions,

        Commands::Kill { s } => Request::Kill {
            session: resolve_session(s),
        },

        Commands::Stop => Request::Stop,

        Commands::Daemon | Commands::Examples | Commands::Pipe => unreachable!(),
    };

    match client::send_request(&request) {
        Ok(response) => print_response(&response, raw_text_output),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn print_response(response: &Response, raw_text: bool) {
    if response.ok {
        if let Some(data) = &response.data {
            if raw_text {
                if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
                    println!("{text}");
                    return;
                }
            }
            println!("{}", serde_json::to_string_pretty(data).unwrap());
        }
    } else {
        if let Some(error) = &response.error {
            let output = serde_json::to_string_pretty(error).unwrap();
            eprintln!("{output}");
        }
        std::process::exit(1);
    }
}

fn print_examples() {
    println!(
        r#"
  Quick Start:
    frameterm spawn vim file.txt
    frameterm wait-for "file.txt"
    frameterm snapshot --format text
    frameterm key i
    frameterm type "Hello, World!"
    frameterm key Escape
    frameterm key ": w q Enter"
    frameterm record export --output .
    frameterm kill

  Multiple Sessions:
    frameterm spawn --name editor vim file.txt
    frameterm spawn --name monitor htop
    frameterm snapshot -s editor --format text
    frameterm key -s monitor q
    frameterm record export --all

  Screen Change Detection:
    HASH=$(frameterm snapshot | jq -r '.content_hash')
    frameterm key Enter
    frameterm snapshot --await-change $HASH --settle 100
"#
    );
}
