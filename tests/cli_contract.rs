use std::process::{Command, Stdio};

fn app() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sagebrush"));
    command.env("PATH", "").env_remove("TMUX");
    command
}

#[test]
fn root_and_subcommand_help_explain_the_contract_without_tmux() {
    for (args, required) in [
        (vec!["--help"], vec!["tui", "list", "--no-color"]),
        (
            vec!["list", "--help"],
            vec![
                "--json",
                "--plain",
                "uptime",
                "invoking client's session",
                "outside tmux",
                "active status is unavailable",
            ],
        ),
        (
            vec!["tui", "--help"],
            vec!["terminal", "press ?", "j/k", "esc", "q to quit", "crawl"],
        ),
    ] {
        let output = app().args(&args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        let help = String::from_utf8(output.stdout).unwrap().to_lowercase();
        for item in required {
            assert!(help.contains(item), "{args:?}: missing {item}: {help}");
        }
        assert!(output.stderr.is_empty());
    }
    assert!(app().arg("--version").output().unwrap().status.success());
}

#[test]
fn invalid_arguments_and_nonterminal_tui_return_usage_error() {
    for args in [
        vec!["list", "--json", "--plain"],
        vec!["list", "--unknown"],
        vec!["unknown"],
        vec!["tui", "extra"],
        vec![],
    ] {
        let output = app().args(&args).stdin(Stdio::null()).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn no_color_is_global_before_or_after_subcommands() {
    for args in [
        vec!["--no-color", "list", "--help"],
        vec!["list", "--no-color", "--help"],
        vec!["tui", "--no-color", "--help"],
    ] {
        let output = app().args(&args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
}
