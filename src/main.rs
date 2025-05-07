// main.rs

use std::process::{Command, Stdio};
use std::str;

// --- 型定義 ---
// CommandResult は main.rs で定義し、cmds.rs から crate::CommandResult として参照
pub type CommandResult<T> = Result<T, String>;
// CommandHandler も main.rs で定義し、cmds.rs から crate::CommandHandler として参照
pub type CommandHandler = fn(&[String]);

pub struct CommandDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: CommandHandler,
}

// --- 低レベルなGitコマンド実行ヘルパー ---
fn execute_git_command_internal(args: &[&str], capture_stdout: bool, description: &str) -> CommandResult<String> {
    let mut command = Command::new("git");
    command.args(args);

    let output_res = if capture_stdout {
        command.stderr(Stdio::piped()).output()
    } else {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit()).output()
    };

    match output_res {
        Ok(output) => {
            if output.status.success() {
                if capture_stdout {
                    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    Ok(String::new())
                }
            } else {
                let mut err_msg = format!("エラー: コマンド \"{}\" 失敗 (コード: {})", description, output.status.code().unwrap_or(-1));
                if !output.stderr.is_empty() {
                    err_msg.push_str(&format!("\nstderr:\n{}", String::from_utf8_lossy(&output.stderr).trim()));
                }
                if capture_stdout && !output.stdout.is_empty() && !output.status.success() {
                     err_msg.push_str(&format!("\nstdout:\n{}", String::from_utf8_lossy(&output.stdout).trim()));
                }
                Err(err_msg)
            }
        }
        Err(e) => {
            Err(format!("エラー: コマンド \"{}\" の実行に失敗しました。詳細: {}", description, e))
        }
    }
}

pub struct GitCommand;
impl GitCommand {
    fn run_interactive(args: &[&str], cmd_description: &str) -> CommandResult<()> {
        execute_git_command_internal(args, false, cmd_description).map(|_| ())
    }
    fn run_stdout(args: &[&str], cmd_description: &str) -> CommandResult<String> {
        execute_git_command_internal(args, true, cmd_description)
    }
    fn run_check_exit_code_zero(args: &[&str], cmd_description: &str) -> CommandResult<bool> {
        match Command::new("git").args(args).stdout(Stdio::null()).stderr(Stdio::null()).status() {
            Ok(status) => Ok(status.success()),
            Err(e) => Err(format!("コマンド \"{}\" の状態確認に失敗: {}", cmd_description, e)),
        }
    }

    pub fn init() -> CommandResult<()> { Self::run_interactive(&["init"], "git init") }
    pub fn remote_add(remote: &str, url: &str) -> CommandResult<()> { Self::run_interactive(&["remote", "add", remote, url], "git remote add") }
    pub fn remote_set_url(remote: &str, url: &str) -> CommandResult<()> { Self::run_interactive(&["remote", "set-url", remote, url], "git remote set-url") }
    pub fn remote_remove(remote: &str) -> CommandResult<()> { Self::run_interactive(&["remote", "remove", remote], "git remote remove")}
    pub fn remote_get_url(remote: &str) -> CommandResult<String> { Self::run_stdout(&["remote", "get-url", remote], "git remote get-url") }
    pub fn remote_list_str() -> CommandResult<String> { Self::run_stdout(&["remote"], "git remote") }
    
    pub fn add(files: &str) -> CommandResult<()> { Self::run_interactive(&["add", files], "git add") }
    pub fn commit(message: &str) -> CommandResult<()> { Self::run_interactive(&["commit", "-m", message], "git commit") }
    pub fn push(remote: &str, branch: &str) -> CommandResult<()> { Self::run_interactive(&["push", remote, branch], "git push") }
    pub fn push_u(remote: &str, branch: &str) -> CommandResult<()> { Self::run_interactive(&["push", "-u", remote, branch], "git push -u") }
    pub fn push_delete(remote: &str, branch: &str) -> CommandResult<()> { Self::run_interactive(&["push", remote, "--delete", branch], "git push --delete") }
    pub fn push_ref_to_ref(remote: &str, source_and_dest_ref: &str) -> CommandResult<()> {
        Self::run_interactive(&["push", remote, source_and_dest_ref], "git push <ref>:<ref>")
    }
        
    pub fn branch_list_all_str() -> CommandResult<String> { Self::run_stdout(&["branch", "--all", "--no-color"], "git branch --all")}
    pub fn branch_list_local_str() -> CommandResult<String> { Self::run_stdout(&["branch", "--no-color"], "git branch")}
    pub fn branch_create_local(name: &str) -> CommandResult<()> { Self::run_interactive(&["branch", name], "git branch <name>") }
    pub fn branch_create_local_from(name: &str, source: &str) -> CommandResult<()> { Self::run_interactive(&["branch", name, source], "git branch <name> <source>") }
    pub fn branch_delete_local_d(branch: &str) -> CommandResult<()> { Self::run_interactive(&["branch", "-d", branch], "git branch -d") }

    pub fn checkout(branch: &str) -> CommandResult<()> { Self::run_interactive(&["checkout", branch], "git checkout") }
    pub fn checkout_b(branch: &str) -> CommandResult<()> { Self::run_interactive(&["checkout", "-b", branch], "git checkout -b") }
    
    pub fn merge(branch: &str) -> CommandResult<bool> { Self::run_check_exit_code_zero(&["merge", branch], "git merge") }
    pub fn pull(remote: &str, branch: &str) -> CommandResult<bool> { 
        Self::run_check_exit_code_zero(&["pull", remote, branch], "git pull (check)")
    }
    
    pub fn fetch_prune(remote: &str) -> CommandResult<()> { Self::run_interactive(&["fetch", remote, "--prune"], "git fetch --prune") }
    
    pub fn symbolic_ref_head() -> CommandResult<String> {
        let result = Self::run_stdout(&["symbolic-ref", "--short", "-q", "HEAD"], "git symbolic-ref --short HEAD")?;
        if result == "HEAD" { return Ok(String::new()); }
        Ok(result)
    }
    pub fn config_get(key: &str) -> CommandResult<String> {
        Self::run_stdout(&["config", key], &format!("git config {}", key))
    }
    pub fn rev_parse_verify(ref_name: &str) -> CommandResult<bool> {
        Self::run_check_exit_code_zero(&["rev-parse", "--verify", "--quiet", ref_name], "git rev-parse --verify")
    }
    pub fn rev_parse_commit_id(ref_name: &str) -> CommandResult<String> {
        Self::run_stdout(&["rev-parse", ref_name], "git rev-parse")
    }
    pub fn status_porcelain_v1() -> CommandResult<String> {
        Self::run_stdout(&["status", "--porcelain"], "git status --porcelain")
    }
    pub fn merge_base(commit1: &str, commit2: &str) -> CommandResult<String> {
        Self::run_stdout(&["merge-base", commit1, commit2], "git merge-base")
    }
}

// COMMAND_DEFINITIONS は pub const にして、cmds.rs から crate::COMMAND_DEFINITIONS で参照
pub const COMMAND_DEFINITIONS: &[CommandDefinition] = &[
    CommandDefinition { name: "save", description: "現在の変更を記録し、オプションでリモートに保存します。", handler: cmds::git_save },
    CommandDefinition { name: "setup", description: "リポジトリの初期化とリモート('origin')の接続設定を行います。", handler: cmds::git_setup },
    CommandDefinition { name: "branch", description: "ブランチの一覧を状態に応じて色分け表示します。", handler: cmds::git_branch },
    CommandDefinition { name: "switch", description: "既存のローカルブランチに切り替えます。", handler: cmds::git_switch },
    CommandDefinition { name: "merge", description: "指定ブランチを現在のブランチにマージします。", handler: cmds::git_merge },
    CommandDefinition { name: "copy", description: "ブランチをローカルにコピーし、オプションでリモートにプッシュします。", handler: cmds::git_copy },
    CommandDefinition { name: "delete", description: "ローカルおよびオプションでリモートブランチを削除します。", handler: cmds::git_delete },
    CommandDefinition { name: "create", description: "新しいローカルブランチを作成し、オプションでリモートにプッシュします。", handler: cmds::git_create },
    CommandDefinition { name: "help", description: "このヘルプメッセージを表示します。", handler: cmds::show_help },
];

mod cmds;
// use cmds::CommandHandler; // CommandHandler は main.rs で pub type となったので不要

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program_name = args.first().map_or("mygit", |s| s.as_str());

    if args.len() < 2 {
        cmds::print_usage_and_exit(program_name, COMMAND_DEFINITIONS);
        return;
    }

    let command_name = args[1].as_str();

    for cmd_def in COMMAND_DEFINITIONS {
        if cmd_def.name == command_name {
            (cmd_def.handler)(&args);
            return;
        }
    }

    eprintln!("エラー: 不明なコマンド '{}'", command_name);
    cmds::print_usage_and_exit(program_name, COMMAND_DEFINITIONS);
}