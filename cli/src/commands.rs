use std::error::Error;

use crate::config::{load_app_config, save_app_config};

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    RunApp,
    PrintHelp(String),
    SetStartupCommand(String),
    ClearStartupCommand,
}

pub enum CommandResult {
    ContinueToApp,
    Exit,
}

pub fn handle_cli_args<I>(args: I) -> Result<CommandResult, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    execute_cli_action(parse_cli_action(args)?)
}

fn execute_cli_action(action: CliAction) -> Result<CommandResult, Box<dyn Error>> {
    match action {
        CliAction::RunApp => Ok(CommandResult::ContinueToApp),
        CliAction::PrintHelp(help) => {
            print!("{}", help);
            Ok(CommandResult::Exit)
        }
        CliAction::SetStartupCommand(command) => {
            save_startup_command(Some(command))?;
            Ok(CommandResult::Exit)
        }
        CliAction::ClearStartupCommand => {
            save_startup_command(None)?;
            Ok(CommandResult::Exit)
        }
    }
}

fn cli_help(bin: &str) -> String {
    format!(
        "\
Beehive TUI

Usage:
  {bin}
  {bin} --startup-cmd \"<command>\"
  {bin} --startup-cmd ''
  {bin} --help

Options:
  --startup-cmd <command>  Save the comb startup command in ~/.beehive/config.json
                           Runs once per comb when that comb first opens after launch
                           Pass an empty string to clear it
  -h, --help               Show this help text
"
    )
}

fn parse_cli_action<I>(args: I) -> Result<CliAction, Box<dyn Error>>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let bin = args.next().unwrap_or_else(|| "beehive".to_string());
    let mut action = CliAction::RunApp;

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Ok(CliAction::PrintHelp(cli_help(&bin)));
        }

        if arg == "--startup-cmd" {
            let value = args.next().ok_or_else(|| {
                format!(
                    "Missing value for --startup-cmd\nUsage: {} --startup-cmd \"<command>\"",
                    bin
                )
            })?;
            action = startup_command_action(value);
            continue;
        }

        if let Some(value) = arg.strip_prefix("--startup-cmd=") {
            action = startup_command_action(value.to_string());
            continue;
        }

        return Err(format!("Unknown argument '{}'\n\n{}", arg, cli_help(&bin)).into());
    }

    Ok(action)
}

fn startup_command_action(value: String) -> CliAction {
    if value.trim().is_empty() {
        CliAction::ClearStartupCommand
    } else {
        CliAction::SetStartupCommand(value)
    }
}

fn save_startup_command(command: Option<String>) -> Result<(), Box<dyn Error>> {
    let mut config = load_app_config().map_err(|e| -> Box<dyn Error> { e.into() })?;
    config.comb_startup_command = command;
    save_app_config(&config).map_err(|e| -> Box<dyn Error> { e.into() })?;

    match config.comb_startup_command.as_deref() {
        Some(command) => println!("Saved comb startup command: {}", command),
        None => println!("Cleared comb startup command"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cli_action_defaults_to_running_app() {
        let action = parse_cli_action(vec!["bh".to_string()]).unwrap();
        assert_eq!(action, CliAction::RunApp);
    }

    #[test]
    fn parse_cli_action_accepts_help_flag() {
        let action = parse_cli_action(vec!["bh".to_string(), "--help".to_string()]).unwrap();

        match action {
            CliAction::PrintHelp(help) => {
                assert!(help.contains("Usage:"));
                assert!(help.contains("--startup-cmd"));
            }
            _ => panic!("expected help action"),
        }
    }

    #[test]
    fn parse_cli_action_accepts_short_help_flag() {
        let action = parse_cli_action(vec!["bh".to_string(), "-h".to_string()]).unwrap();

        match action {
            CliAction::PrintHelp(help) => assert!(help.contains("Beehive TUI")),
            _ => panic!("expected help action"),
        }
    }

    #[test]
    fn parse_cli_action_accepts_startup_command_flag() {
        let action = parse_cli_action(vec![
            "bh".to_string(),
            "--startup-cmd".to_string(),
            "tmux new-session -A -s dev".to_string(),
        ])
        .unwrap();

        assert_eq!(
            action,
            CliAction::SetStartupCommand("tmux new-session -A -s dev".to_string())
        );
    }

    #[test]
    fn parse_cli_action_accepts_equals_form() {
        let action = parse_cli_action(vec![
            "bh".to_string(),
            "--startup-cmd=tmux new-session -A -s dev".to_string(),
        ])
        .unwrap();

        assert_eq!(
            action,
            CliAction::SetStartupCommand("tmux new-session -A -s dev".to_string())
        );
    }

    #[test]
    fn parse_cli_action_treats_empty_command_as_clear() {
        let action = parse_cli_action(vec![
            "bh".to_string(),
            "--startup-cmd".to_string(),
            "".to_string(),
        ])
        .unwrap();

        assert_eq!(action, CliAction::ClearStartupCommand);
    }

    #[test]
    fn parse_cli_action_rejects_missing_startup_command_value() {
        let err =
            parse_cli_action(vec!["bh".to_string(), "--startup-cmd".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Missing value for --startup-cmd"));
    }

    #[test]
    fn parse_cli_action_unknown_argument_includes_help() {
        let err = parse_cli_action(vec!["bh".to_string(), "--wat".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Unknown argument '--wat'"));
        assert!(err.to_string().contains("--help"));
    }
}
