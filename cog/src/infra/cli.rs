//! Argument parsing → `Invocation`. Manual parsing (no dependency): the grammar
//! is small and stable.
//!
//! Form: `cog [--store <path>] <group> <subcommand> [args...]`
//!
//! Usage concerns (help, missing/unknown commands or args) are NOT domain or
//! technical errors — they are a separate `Usage` channel. The composition root
//! prints them as plain text on stderr and exits 64, keeping stdout JSON pure.

pub struct Invocation {
    pub store: String,
    pub command: Command,
}

pub enum Command {
    LogAdd { stream: String, payload: String },
    LogQuery { stream: String },
    FsmDefine { name: String, def_json: String, context_json: Option<String> },
    FsmTransition { name: String, to: String, context_json: Option<String> },
    FsmState { name: String },
}

/// Outcome of parsing that is neither a runnable command nor a domain/technical
/// error: either an explicit help request, or a malformed invocation.
pub enum Usage {
    /// `-h`/`--help` (or a bare group): print this help, exit 0.
    Help(&'static str),
    /// Something is missing or wrong: print one sentence + contextual help, exit 64.
    Error { sentence: String, help: &'static str },
}

const DEFAULT_STORE: &str = ".cog/state.db";

pub const HELP_TOP: &str = "\
cog — external persistent memory (SQLite) for skills/agents

USAGE:
    cog [--store <path>] <command>

GLOBAL OPTIONS:
    --store <path>    database file (default: .cog/state.db)
    -h, --help        show this help (works after any command too)

COMMANDS:
    log     append-only journal per stream
    fsm     state machine with data-defined rules

Run `cog <command> --help` for command-specific help, e.g. `cog log --help`.

Every command prints a JSON result on stdout: {\"ok\":true,\"value\":...} or
{\"ok\":false,\"error\":{\"kind\":...,\"code\":...}}. Usage/help text goes to stderr.
Exit codes: 0 ok, 2 domain error, 64 usage error, 70 technical error.";

pub const HELP_LOG: &str = "\
cog log — append-only journal per stream

USAGE:
    cog log add <stream> <json>    append a JSON entry to a stream
    cog log query <stream>         list a stream's entries, newest first

EXAMPLES:
    # Wrap JSON in SINGLE quotes so the shell leaves it untouched:
    cog log add reviews '{\"comment\": 42}'
    cog log query reviews";

pub const HELP_FSM: &str = "\
cog fsm — state machine whose states & transitions are data

USAGE:
    cog fsm define <name> <def-json> [--context <json>]   define a machine
    cog fsm transition <name> <state> [--context <json>]   move to a new state
    cog fsm state <name>                                   show current + context

The definition lists states (each with optional terminal/description) and the
allowed transitions; the machine starts at \"initial\".

--context carries an opaque JSON blob (e.g. a poll cursor) alongside the state.
cog never inspects its shape. On `define` it is the machine's initial context
(default: null); on `transition` it WHOLLY replaces the blob in the same
transaction as the move (omit it to leave the blob untouched). `state` returns it.

EXAMPLES:
    cog fsm define watch '{\"states\":[{\"name\":\"idle\"},{\"name\":\"done\",\"terminal\":true}],\"transitions\":[{\"from\":\"idle\",\"to\":\"done\"}],\"initial\":\"idle\"}'
    cog fsm transition watch done --context '{\"last_seen\":42}'
    cog fsm state watch";

/// Pick the most specific help for what the user has typed so far.
fn help_for(group: &str) -> &'static str {
    match group {
        "log" => HELP_LOG,
        "fsm" => HELP_FSM,
        _ => HELP_TOP,
    }
}

pub fn parse(args: &[String]) -> Result<Invocation, Usage> {
    let mut store = DEFAULT_STORE.to_string();
    let mut help = false;
    let mut context_json: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--store" => {
                i += 1;
                store = args
                    .get(i)
                    .ok_or_else(|| Usage::Error {
                        sentence: "Option --store expects a path.".to_string(),
                        help: HELP_TOP,
                    })?
                    .clone();
            }
            "--context" => {
                i += 1;
                context_json = Some(
                    args.get(i)
                        .ok_or_else(|| Usage::Error {
                            sentence: "Option --context expects a JSON blob.".to_string(),
                            help: HELP_FSM,
                        })?
                        .clone(),
                );
            }
            "-h" | "--help" => help = true,
            _ => rest.push(args[i].clone()),
        }
        i += 1;
    }

    let group = rest.first().map(String::as_str).unwrap_or("");
    let sub = rest.get(1).map(String::as_str).unwrap_or("");

    // Explicit help wins over everything, scoped to the group typed so far.
    if help {
        return Err(Usage::Help(help_for(group)));
    }

    let command = match (group, sub) {
        ("log", "add") => Command::LogAdd {
            stream: req(&rest, 2, "stream", HELP_LOG)?,
            payload: req(&rest, 3, "json payload", HELP_LOG)?,
        },
        ("log", "query") => Command::LogQuery {
            stream: req(&rest, 2, "stream", HELP_LOG)?,
        },
        ("log", _) => return Err(missing_sub("log", "add or query", HELP_LOG)),

        ("fsm", "define") => Command::FsmDefine {
            name: req(&rest, 2, "name", HELP_FSM)?,
            def_json: req(&rest, 3, "json definition", HELP_FSM)?,
            context_json,
        },
        ("fsm", "transition") => Command::FsmTransition {
            name: req(&rest, 2, "name", HELP_FSM)?,
            to: req(&rest, 3, "target state", HELP_FSM)?,
            context_json,
        },
        ("fsm", "state") => Command::FsmState {
            name: req(&rest, 2, "name", HELP_FSM)?,
        },
        ("fsm", _) => return Err(missing_sub("fsm", "define, transition or state", HELP_FSM)),

        ("", _) => {
            return Err(Usage::Error {
                sentence: "No command given.".to_string(),
                help: HELP_TOP,
            })
        }
        _ => {
            return Err(Usage::Error {
                sentence: format!("Unknown command '{group}'."),
                help: HELP_TOP,
            })
        }
    };

    Ok(Invocation { store, command })
}

fn req(rest: &[String], idx: usize, name: &str, help: &'static str) -> Result<String, Usage> {
    rest.get(idx).cloned().ok_or_else(|| Usage::Error {
        sentence: format!("Missing argument: {name}."),
        help,
    })
}

fn missing_sub(group: &str, choices: &str, help: &'static str) -> Usage {
    Usage::Error {
        sentence: format!("Command '{group}' needs a subcommand ({choices})."),
        help,
    }
}
