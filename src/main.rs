use clap::Parser;
use std::process::{Command, Stdio};
use std::str;
use crate::utils::GitCommandTrait;

mod cmds;
mod utils;

// --- Gitコマンド実行のコアロジック (変更点はanyhow::Result化) ---
// CommandResult は anyhow::Result に置き換えられるため、グローバルな型エイリアスは不要になることが多い
// pub type CommandResult<T> = Result<T, String>; // anyhow で代替

pub struct GitCommand; // utils.rs の GitCommandTrait の実装はここで行う
impl GitCommand {
    fn execute_git_command_internal(args: &[&str], capture_stdout: bool, description: &str) -> anyhow::Result<String> {
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
                    let code = output.status.code().unwrap_or(-1);
                    let mut err_msg_parts = vec![format!("エラー: コマンド \"{}\" 失敗 (コード: {})", description, code)];
                    if !output.stderr.is_empty() {
                        err_msg_parts.push(format!("stderr:\n{}", String::from_utf8_lossy(&output.stderr).trim()));
                    }
                    if capture_stdout && !output.stdout.is_empty() {
                        err_msg_parts.push(format!("stdout:\n{}", String::from_utf8_lossy(&output.stdout).trim()));
                    }
                    anyhow::bail!(err_msg_parts.join("\n"))
                }
            }
            Err(e) => Err(anyhow::Error::new(e).context(format!("コマンド \"{}\" の実行に失敗", description))),
        }
    }

    // --- GitCommand のヘルパーメソッド群 (戻り値を anyhow::Result に変更) ---
    fn run_interactive(args: &[&str], cmd_description: &str) -> anyhow::Result<()> {
        Self::execute_git_command_internal(args, false, cmd_description).map(|_| ())
    }
    fn run_stdout(args: &[&str], cmd_description: &str) -> anyhow::Result<String> {
        Self::execute_git_command_internal(args, true, cmd_description)
    }
    fn run_check_exit_code_zero(args: &[&str], cmd_description: &str) -> anyhow::Result<bool> {
        match Command::new("git").args(args).stdout(Stdio::null()).stderr(Stdio::null()).status() {
            Ok(status) => Ok(status.success()),
            Err(e) => Err(anyhow::Error::new(e).context(format!("コマンド \"{}\" の状態確認に失敗", cmd_description))),
        }
    }

    // --- 各Git操作メソッド (戻り値を anyhow::Result に変更) ---
    pub fn init() -> anyhow::Result<()> { Self::run_interactive(&["init"], "git init") }
    pub fn remote_add(remote: &str, url: &str) -> anyhow::Result<()> { Self::run_interactive(&["remote", "add", remote, url], "git remote add") }
    pub fn remote_set_url(remote: &str, url: &str) -> anyhow::Result<()> { Self::run_interactive(&["remote", "set-url", remote, url], "git remote set-url") }
    pub fn remote_remove(remote: &str) -> anyhow::Result<()> { Self::run_interactive(&["remote", "remove", remote], "git remote remove")}
    pub fn remote_get_url(remote: &str) -> anyhow::Result<String> { Self::run_stdout(&["remote", "get-url", remote], "git remote get-url") }
    
    pub fn add(files: &str) -> anyhow::Result<()> { Self::run_interactive(&["add", files], "git add") }
    pub fn commit(message: &str) -> anyhow::Result<()> { Self::run_interactive(&["commit", "-m", message], "git commit") }
    pub fn push_u(remote: &str, branch: &str) -> anyhow::Result<()> { Self::run_interactive(&["push", "-u", remote, branch], "git push -u") }
    pub fn push_delete(remote: &str, branch: &str) -> anyhow::Result<()> { Self::run_interactive(&["push", remote, "--delete", branch], "git push --delete") }
        
    pub fn branch_list_all_str() -> anyhow::Result<String> { Self::run_stdout(&["branch", "--all", "--no-color"], "git branch --all")}
    pub fn branch_list_local_str() -> anyhow::Result<String> { Self::run_stdout(&["branch", "--no-color"], "git branch")}
    pub fn branch_create_local(name: &str) -> anyhow::Result<()> { Self::run_interactive(&["branch", name], "git branch <name>") }
    pub fn branch_create_local_from(name: &str, source: &str) -> anyhow::Result<()> { Self::run_interactive(&["branch", name, source], "git branch <name> <source>") }
    pub fn branch_delete_local_d(branch: &str) -> anyhow::Result<()> { Self::run_interactive(&["branch", "-d", branch], "git branch -d") }

    pub fn checkout(branch: &str) -> anyhow::Result<()> { Self::run_interactive(&["checkout", branch], "git checkout") }
    pub fn checkout_b(branch: &str) -> anyhow::Result<()> { Self::run_interactive(&["checkout", "-b", branch], "git checkout -b") }
    
    pub fn merge(branch: &str) -> anyhow::Result<bool> { Self::run_check_exit_code_zero(&["merge", branch], "git merge") }
    pub fn pull(remote: &str, branch: &str) -> anyhow::Result<bool> { 
        Self::run_check_exit_code_zero(&["pull", remote, branch], "git pull (check)")
    }
    pub fn fetch_prune(remote: &str) -> anyhow::Result<()> { Self::run_interactive(&["fetch", remote, "--prune"], "git fetch --prune") }
    
    pub fn symbolic_ref_head() -> anyhow::Result<String> {
        let result = Self::run_stdout(&["symbolic-ref", "--short", "-q", "HEAD"], "git symbolic-ref --short HEAD")?;
        if result == "HEAD" { return Ok(String::new()); }
        Ok(result)
    }
    pub fn rev_parse_verify(ref_name: &str) -> anyhow::Result<bool> {
        Self::run_check_exit_code_zero(&["rev-parse", "--verify", "--quiet", ref_name], "git rev-parse --verify")
    }
    pub fn rev_parse_commit_id(ref_name: &str) -> anyhow::Result<String> {
        Self::run_stdout(&["rev-parse", ref_name], "git rev-parse")
    }
    pub fn status_porcelain_v1() -> anyhow::Result<String> {
        Self::run_stdout(&["status", "--porcelain"], "git status --porcelain")
    }
    pub fn merge_base(commit1: &str, commit2: &str) -> anyhow::Result<String> {
        Self::run_stdout(&["merge-base", commit1, commit2], "git merge-base")
    }
    pub fn show_branch_tree() -> anyhow::Result<String> {
        Self::run_stdout(&["show-branch", "--list", "--topo-order"], "git show-branch --list --topo-order")
    }
}

impl GitCommandTrait for GitCommand {
    fn rev_parse_verify(&self, ref_name: &str) -> Result<bool, anyhow::Error> {
        GitCommand::rev_parse_verify(ref_name)
    }

    fn rev_parse_commit_id(&self, ref_name: &str) -> Result<String, anyhow::Error> {
        GitCommand::rev_parse_commit_id(ref_name)
    }

    fn merge_base(&self, commit1: &str, commit2: &str) -> Result<String, anyhow::Error> {
        GitCommand::merge_base(commit1, commit2)
    }

    fn symbolic_ref_head(&self) -> Result<String, anyhow::Error> {
        GitCommand::symbolic_ref_head()
    }

    fn checkout(&self, branch_name: &str) -> Result<(), anyhow::Error> {
        GitCommand::checkout(branch_name)
    }

    fn checkout_b(&self, branch_name: &str) -> Result<(), anyhow::Error> {
        GitCommand::checkout_b(branch_name)
    }

    fn push_u(&self, remote: &str, branch: &str) -> Result<(), anyhow::Error> {
        GitCommand::push_u(remote, branch)
    }

    fn remote_get_url(&self, remote: &str) -> Result<String, anyhow::Error> {
        GitCommand::remote_get_url(remote)
    }
}

// --- clapによるコマンド定義 ---
#[derive(Parser, Debug)]
#[clap(author, version, about = "Git操作を簡略化する個人用CLIツール", long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// 現在の変更を記録し、オプションでリモートに保存 (エイリアス: sa)
    #[clap(alias = "sa")]
    Save(cmds::SaveArgs),
    /// ブランチの一覧を状態に応じて色分け表示 (エイリアス: br)
    #[clap(alias = "br")]
    Branch(cmds::BranchArgs),
    /// 既存のブランチに切り替え、またはリモートブランチから新規作成して切り替え (エイリアス: sw)
    #[clap(alias = "sw")]
    Switch(cmds::SwitchArgs),
    /// 指定ブランチを現在のブランチにマージ (エイリアス: me)
    #[clap(alias = "me")]
    Merge(cmds::MergeArgs),
    /// ブランチをローカルにコピーし、オプションでリモートにプッシュ (エイリアス: cp)
    #[clap(alias = "cp")]
    Copy(cmds::CopyArgs),
    /// ローカルおよびオプションでリモートブランチを削除 (エイリアス: del)
    #[clap(alias = "del")]
    Delete(cmds::DeleteArgs),
    /// 新しいローカルブランチを作成し、オプションでリモートにプッシュ (エイリアス: cr)
    #[clap(alias = "cr")]
    Create(cmds::CreateArgs),
    /// ブランチの履歴をツリー形式で表示 (エイリアス: tr)
    #[clap(alias = "tr")]
    Tree(cmds::TreeArgs),
    /// リポジトリ関連の操作 (初期化、作成、削除、リモート設定)
    Repo(cmds::RepoArgsCli), // cmds.rs で RepoArgsCli とその中のサブコマンドを定義
    // clap が自動で help サブコマンドを生成するので、自前のものは不要になることが多い
    // Help,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Save(args) => cmds::git_save(args)?,
        Commands::Branch(args) => cmds::git_branch(args)?,
        Commands::Switch(args) => cmds::git_switch(args)?,
        Commands::Merge(args) => cmds::git_merge(args)?,
        Commands::Copy(args) => cmds::git_copy(args)?,
        Commands::Delete(args) => cmds::git_delete(args)?,
        Commands::Create(args) => cmds::git_create(args)?,
        Commands::Tree(args) => cmds::git_tree(args)?,
        Commands::Repo(args) => cmds::handle_repo_command(args)?,
    }

    Ok(())
}