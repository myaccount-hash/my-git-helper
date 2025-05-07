use anyhow::{bail, Context, Result};
use clap::Parser; // clap の Args とかはここに定義
use colored::*;
use crate::{
    GitCommand,
    utils::{
        // display_branch_list, // FuzzySelect に置き換えられるので不要になることが多い
        ensure_branch_exists, ensure_branch_not_exists, get_branch_select_options_for_fuzzy,
        get_current_branch_name, get_origin_url, prompt_and_push_optional, prompt_confirm,
        prompt_fuzzy_select, prompt_input, prompt_non_empty_input, resolve_branch_input,
        get_branch_display_info, handle_conflict_and_offer_new_branch,
        SelectOption,
    },
};

// --- clap で使う各コマンドの引数用構造体 ---
#[derive(Parser, Debug)]
pub struct SaveArgs {
    // save コマンドに特有な引数があればここに定義
    // 例: #[clap(short, long)] message: Option<String>,
}

#[derive(Parser, Debug)]
pub struct BranchArgs {
    // branch コマンドに特有な引数
}

#[derive(Parser, Debug)]
pub struct SwitchArgs {
    // switch コマンドに特有な引数 (ブランチ名は対話的に聞くので不要かも)
    // pub branch_name: Option<String>, // もし引数でも渡せるようにするなら
}

#[derive(Parser, Debug)]
pub struct MergeArgs {
    // pub branch_name: Option<String>, // 対話的に聞く
}

#[derive(Parser, Debug)]
pub struct CopyArgs {
    // pub source_branch: Option<String>, // 対話的
    // pub new_branch_name: Option<String>, // これも対話的か、あるいは必須引数
}

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    // pub branch_name: Option<String>, // 対話的
}

#[derive(Parser, Debug)]
pub struct CreateArgs {
    // pub branch_name: Option<String>, // 新しいブランチ名は入力させる方が良いか
}

#[derive(Parser, Debug)]
pub struct TreeArgs {}


// --- リポジトリ管理コマンド (repo) の定義 ---
#[derive(Parser, Debug)]
pub struct RepoArgsCli {
    #[clap(subcommand)]
    pub command: Option<RepoCommands>, // サブコマンドがない場合は対話的に選択
}

#[derive(clap::Subcommand, Debug)]
pub enum RepoCommands {
    /// カレントディレクトリをGitリポジトリとして初期化します。
    Init,
    /// 新しいディレクトリを作成し、Gitリポジトリとして初期化します。
    Create {
        /// 作成するリポジトリ（ディレクトリ）名
        name: String,
    },
    /// カレントリポジトリの.gitディレクトリを削除します（非常に危険な操作です！）。
    Delete,
    /// リモートリポジトリ ('origin') の設定を管理します。
    Remote {
        #[clap(subcommand)]
        command: Option<RemoteRepoCommands>, // これも省略されたら対話的選択
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum RemoteRepoCommands {
    /// 新しいリモート'origin'を追加します。
    Add { url: String },
    /// リモート'origin'のURLを変更します。
    SetUrl { url: String },
    /// リモート'origin'を削除します。
    Remove,
    /// 現在のリモート'origin'設定を表示します。
    Show,
}


// --- コマンドハンドラ関数 (シグネチャと戻り値を変更) ---

pub fn git_save(_args: SaveArgs) -> Result<()> {
    GitCommand::add(".").context("git add . の実行に失敗")?;
    let msg = prompt_non_empty_input("コミットメッセージ: ", "エラー: メッセージ必須。")?;
    GitCommand::commit(&msg).context(format!("コミット失敗: {}", msg))?;
    println!("ローカルにコミットしました。");

    let current_branch = get_current_branch_name(&GitCommand)?;
    if current_branch.is_empty() {
        println!("{}", "エラー: 現在のブランチ不明。プッシュをスキップ。".yellow());
        return Ok(());
    }

    match get_origin_url(&GitCommand) {
        Ok(remote_url) if !remote_url.is_empty() => {
            let push_prompt = format!("リモート 'origin/{}' にもプッシュしますか？", current_branch);
            if prompt_confirm(&push_prompt)? {
                GitCommand::push_u("origin", &current_branch)
                    .context(format!("'origin/{}' へのプッシュ失敗", current_branch))?;
                println!("'origin/{}' へプッシュしました。", current_branch.cyan());

                if prompt_confirm("リモートの最新の変更をプルしますか？ (コンフリクトの可能性あり)")? {
                    match GitCommand::pull("origin", &current_branch) {
                        Ok(true) => println!("{}", "プル成功。最新の状態です。".green()),
                        Ok(false) => handle_conflict_and_offer_new_branch("プル", &GitCommand)?,
                        Err(e) => return Err(e.context("プル処理中にエラーが発生しました")),
                    }
                }
            } else {
                println!("リモートへのプッシュはスキップしました。");
            }
        }
        _ => {
            println!("{}", "リモート 'origin' が未設定または取得失敗のため、プッシュはスキップしました。".yellow());
        }
    }
    println!("{}", "保存処理が完了しました。".green());
    Ok(())
}

pub fn git_branch(_args: BranchArgs) -> Result<()> {
    let remote_url = get_origin_url(&GitCommand).unwrap_or_default(); // なければ空文字

    if !remote_url.is_empty() {
        GitCommand::fetch_prune("origin").context("git fetch --prune の実行に失敗")?;
        println!("ブランチ一覧 (リモート 'origin' を含む):");
    } else {
        println!("ローカルブランチ一覧 (リモート 'origin' 未設定):");
    }

    let branches_all_str = GitCommand::branch_list_all_str().context("全ブランチリストの取得に失敗")?;
    let uncommitted_changes = !GitCommand::status_porcelain_v1().context("git status の取得に失敗")?.is_empty();
    let mut displayed_locals = std::collections::HashSet::new();

    for line in branches_all_str.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() || trimmed_line.contains(" -> ") || trimmed_line.ends_with("/HEAD") {
            continue;
        }
        let is_current = trimmed_line.starts_with("* ");
        let name_part = trimmed_line.trim_start_matches("* ");
        
        let (display_name, is_remote_line) = if let Some(remote_branch_name) = name_part.strip_prefix("remotes/origin/") {
            (remote_branch_name.to_string(), true)
        } else {
            (name_part.to_string(), false)
        };
        
        if is_remote_line {
            if !displayed_locals.contains(&display_name) {
                 println!("  {} {}", display_name.blue(), "(リモートのみ)".dimmed());
            }
        } else {
            displayed_locals.insert(display_name.clone());
            let (display_str, note) = get_branch_display_info(&display_name, is_current, uncommitted_changes, &remote_url, &GitCommand)?;
            println!("{} {}", display_str, note);
        }
    }
    Ok(())
}

pub fn git_switch(_args: SwitchArgs) -> Result<()> {
    let branch_options = get_branch_select_options_for_fuzzy().context("ブランチ選択肢の取得に失敗")?;
    let target_input = match prompt_fuzzy_select("切り替えたいブランチ:", &branch_options)? {
        Some(selection) => selection,
        None => {
            println!("ブランチ選択がキャンセルされました。");
            return Ok(());
        }
    };

    let resolved = resolve_branch_input(&target_input, &GitCommand)?;

    if resolved.local_exists && resolved.local_candidate_name == target_input {
        GitCommand::checkout(&resolved.local_candidate_name).context(format!("ブランチ '{}' への切り替え失敗", resolved.local_candidate_name))?;
        println!("ブランチ '{}' へ切り替えました。", resolved.local_candidate_name.cyan());
    } else if resolved.remote_tracking_exists {
        let prompt_msg = format!(
            "ローカルブランチ '{}' をリモートブランチ '{}' から作成して切り替えますか？",
            resolved.local_candidate_name.yellow(), resolved.remote_tracking_candidate_name.blue()
        );
        if prompt_confirm(&prompt_msg)? {
            GitCommand::checkout(&resolved.local_candidate_name).context(format!("ブランチ '{}' の作成と切り替え失敗", resolved.local_candidate_name))?;
            println!("ブランチ '{}' を作成し、リモート '{}' を追跡して切り替えました。",
                    resolved.local_candidate_name.cyan(), resolved.remote_tracking_candidate_name.blue());
        } else {
            println!("操作をキャンセルしました。");
        }
    } else {
        bail!("エラー: ブランチ '{}' は見つかりません (ローカルにもリモート'origin'にも対応なし)。", target_input.red());
    }
    Ok(())
}

pub fn git_merge(_args: MergeArgs) -> Result<()> {
    let cur_b = get_current_branch_name(&GitCommand)?;
    if cur_b.is_empty() { bail!("エラー: 現在のブランチが不明です。"); }

    let branch_options = get_branch_select_options_for_fuzzy()?
        .into_iter()
        .filter(|opt| opt.value != cur_b && !opt.value.starts_with(&format!("origin/{}", cur_b)) ) // 自分自身やリモートの自分は除外
        .collect::<Vec<_>>();

    if branch_options.is_empty() {
        println!("{}", "マージ可能なブランチがありません。".yellow());
        return Ok(());
    }
    
    let target = match prompt_fuzzy_select(&format!("ブランチ '{}' にマージするブランチを選択:", cur_b.cyan()), &branch_options)? {
        Some(selection) => selection,
        None => {
            println!("マージ対象の選択がキャンセルされました。");
            return Ok(());
        }
    };
    
    ensure_branch_exists(&target, &GitCommand, "")?;
    
    match GitCommand::merge(&target) {
        Ok(true) => {
            println!("{}", "マージ成功。".green());
            let delete_prompt = format!("マージ元のローカルブランチ '{}' を削除しますか？", target);
            if target.starts_with("origin/") { // リモートブランチを直接マージした場合、ローカル削除は通常しない
                 println!("リモートブランチ '{}' をマージしました。", target.cyan());
            } else if prompt_confirm(&delete_prompt)? {
                GitCommand::branch_delete_local_d(&target).context(format!("ローカルブランチ '{}' の削除失敗", target))?;
                println!("ローカルブランチ '{}' を削除しました。", target.cyan());
            }
        }
        Ok(false) => handle_conflict_and_offer_new_branch("マージ", &GitCommand)?,
        Err(e) => return Err(e.context(format!("ブランチ '{}' のマージ中にエラー", target))),
    }
    Ok(())
}

pub fn git_copy(_args: CopyArgs) -> Result<()> {
    let source_options = get_branch_select_options_for_fuzzy().context("コピー元ブランチ選択肢の取得失敗")?;
    if source_options.is_empty() {
        println!("{}", "コピー可能なブランチがありません。".yellow());
        return Ok(());
    }
    let source = match prompt_fuzzy_select("コピー元ブランチを選択:", &source_options)? {
        Some(selection) => selection,
        None => { println!("コピー元ブランチの選択がキャンセルされました。"); return Ok(()); }
    };
    // ensure_branch_exists は不要（選択肢生成時に存在確認済みのため）

    let new_name = prompt_non_empty_input("新しいブランチ名: ", "エラー: 新ブランチ名必須。")?;
    ensure_branch_not_exists(&new_name, &GitCommand, "ブランチ")?;

    GitCommand::branch_create_local_from(&new_name, &source).context(format!("ブランチ '{}' から '{}' のコピー失敗", source, new_name))?;
    println!("ローカルブランチ '{}' を '{}' からコピーしました。", new_name.cyan(), source.cyan());

    prompt_and_push_optional(&new_name, "コピー", &GitCommand, true)?;
    Ok(())
}

pub fn git_delete(_args: DeleteArgs) -> Result<()> {
    let branch_options = get_branch_select_options_for_fuzzy().context("削除対象ブランチ選択肢の取得失敗")?;
    if branch_options.is_empty() {
        println!("{}", "削除可能なブランチがありません。".yellow());
        return Ok(());
    }
    let target_input = match prompt_fuzzy_select("削除するブランチを選択:", &branch_options)? {
        Some(selection) => selection,
        None => { println!("削除ブランチの選択がキャンセルされました。"); return Ok(());}
    };

    let resolved = resolve_branch_input(&target_input, &GitCommand)?;
    let current_branch_name = get_current_branch_name(&GitCommand)?;
    let remote_url = get_origin_url(&GitCommand).unwrap_or_default();
    let mut something_done = false;

    // ローカルブランチの削除試行
    if resolved.local_exists {
        if current_branch_name == resolved.local_candidate_name {
            bail!("エラー: 現在チェックアウト中のローカルブランチ '{}' は削除できません。", resolved.local_candidate_name.red());
        }
        let local_delete_prompt = format!("ローカルブランチ '{}' を削除しますか？", resolved.local_candidate_name.truecolor(255,165,0));
        if prompt_confirm(&local_delete_prompt)? {
            GitCommand::branch_delete_local_d(&resolved.local_candidate_name).context(format!("ローカルブランチ '{}' の削除失敗", resolved.local_candidate_name))?;
            println!("ローカルブランチ '{}' を削除しました。", resolved.local_candidate_name.truecolor(255,165,0));
            something_done = true;
        }
    }

    // リモートブランチの削除試行 (入力が origin/name だったか、ローカル名でもリモート追跡があれば)
    if resolved.remote_tracking_exists && !remote_url.is_empty() {
         // ユーザーが明示的に `origin/foo` を選択した場合、または `foo` を選択しそれがリモートにもある場合
        let remote_delete_prompt = format!("リモートブランチ '{}' を削除しますか？", resolved.remote_tracking_candidate_name.blue());
        if prompt_confirm(&remote_delete_prompt)? {
            GitCommand::push_delete("origin", &resolved.local_candidate_name).context(format!("リモートブランチ '{}' の削除失敗", resolved.remote_tracking_candidate_name))?;
            println!("リモートブランチ '{}' の削除を試みました。", resolved.remote_tracking_candidate_name.blue());
            something_done = true;
        }
    }
    
    if !something_done {
        // resolved.local_exists と resolved.remote_tracking_exists が両方falseだった場合 (選択肢にあったはずなのに消えた等のレアケース)
        // または両方で「No」を選んだ場合
        println!("ブランチ削除は行われませんでした。");
        if !resolved.local_exists && !resolved.remote_tracking_exists {
             bail!("選択されたブランチ '{}' が見つかりませんでした。", target_input.yellow());
        }
    }
    Ok(())
}

pub fn git_create(_args: CreateArgs) -> Result<()> {
    let name = prompt_non_empty_input("作成する新しいローカルブランチ名: ", "エラー: ブランチ名必須。")?;
    ensure_branch_not_exists(&name, &GitCommand, "ブランチ")?;

    GitCommand::branch_create_local(&name).context(format!("ローカルブランチ '{}' の作成失敗", name))?;
    println!("ローカルブランチ '{}' を作成しました。", name.truecolor(255,165,0));
    
    prompt_and_push_optional(&name, "作成", &GitCommand, true)?;
    Ok(())
}

pub fn git_tree(_args: TreeArgs) -> Result<()> {
    let output = GitCommand::show_branch_tree().context("git show-branch の実行に失敗")?;
    println!("{}", output);
    Ok(())
}

// --- Repo コマンドハンドラ ---
pub fn handle_repo_command(args: RepoArgsCli) -> Result<()> {
    match args.command {
        Some(RepoCommands::Init) => git_repo_init(),
        Some(RepoCommands::Create { name }) => git_repo_create(name),
        Some(RepoCommands::Delete) => git_repo_delete(),
        Some(RepoCommands::Remote { command }) => handle_repo_remote_command(command),
        None => { // `mygit repo` のみでサブコマンドがない場合
            let choices = vec![
                SelectOption { display: "init: カレントディレクトリを初期化".to_string(), value: "init".to_string() },
                SelectOption { display: "create: 新規リポジトリ作成".to_string(), value: "create".to_string() },
                SelectOption { display: "delete: カレントリポジトリの.git削除 (危険!)".to_string(), value: "delete".to_string() },
                SelectOption { display: "remote: リモート設定".to_string(), value: "remote".to_string() },
            ];
            match prompt_fuzzy_select("実行するrepo操作を選択:", &choices)? {
                Some(sub_cmd_str) => {
                    // 選択されたサブコマンドを再度 clap でパースさせるのは少し複雑なので、直接対応する関数を呼ぶ
                    // もしくは、`mygit repo <選択されたサブコマンド>` を再実行する形も考えられる
                    match sub_cmd_str.as_str() {
                        "init" => git_repo_init()?,
                        "create" => {
                            let name = prompt_non_empty_input("作成するリポジトリ名:", "リポジトリ名は必須です。")?;
                            git_repo_create(name)?
                        },
                        "delete" => git_repo_delete()?,
                        "remote" => handle_repo_remote_command(None)?, // さらに選択を促す
                        _ => bail!("不正な選択です。"),
                    }
                }
                None => println!("repo 操作がキャンセルされました。"),
            }
            Ok(())
        }
    }
}

fn git_repo_init() -> Result<()> {
    if std::path::Path::new(".git").exists() {
        if !prompt_confirm("既にGitリポジトリです。再初期化しますか？ (非推奨)")? {
            println!("再初期化はキャンセルされました。");
            return Ok(());
        }
    }
    GitCommand::init().context("git init の実行に失敗")?;
    println!("カレントディレクトリをGitリポジトリとして初期化しました。");
    Ok(())
}

fn git_repo_create(name: String) -> Result<()> {
    if std::path::Path::new(&name).exists() {
        bail!("エラー: ディレクトリ '{}' は既に存在します。", name.red());
    }
    std::fs::create_dir_all(&name).context(format!("ディレクトリ '{}' の作成に失敗", name))?;
    let original_dir = std::env::current_dir().context("カレントディレクトリの取得に失敗")?;
    std::env::set_current_dir(&name).context(format!("ディレクトリ '{}' への移動に失敗", name))?;
    
    let init_result = GitCommand::init(); // anyhow::Result を返す
    
    std::env::set_current_dir(&original_dir).context(format!("元のディレクトリ '{}' への復帰に失敗", original_dir.display()))?;
    
    init_result.context(format!("リポジトリ '{}' のgit initに失敗", name))?; // initの結果をチェック
    println!("リポジトリ '{}' を作成し初期化しました。", name.cyan());
    Ok(())
}

fn git_repo_delete() -> Result<()> {
    if !std::path::Path::new(".git").exists() {
        bail!("エラー: カレントディレクトリはGitリポジトリではありません。");
    }
    println!("{}", "警告: この操作はカレントディレクトリの.gitフォルダを完全に削除し、元に戻せません！".yellow().bold());
    println!("{}", "作業ツリーのファイルは削除されませんが、Gitの履歴やブランチ情報は失われます。".yellow());

    let repo_name = std::env::current_dir()?.file_name().unwrap_or_default().to_string_lossy().to_string();
    let confirm_prompt1 = format!("本当にリポジトリ '{}' の.gitフォルダを削除しますか？", repo_name.cyan());
    if !prompt_confirm(&confirm_prompt1)? {
        println!("削除はキャンセルされました。");
        return Ok(());
    }
    let confirm_prompt2 = format!("最終確認です。'{}' と入力して削除を確定してください:", repo_name);
    let typed_confirmation = prompt_input(&confirm_prompt2)?;
    if typed_confirmation != repo_name {
        println!("確認文字列が一致しませんでした。削除はキャンセルされました。");
        return Ok(());
    }

    std::fs::remove_dir_all(".git").context(".git ディレクトリの削除に失敗")?;
    println!("リポジトリ '{}' (.git ディレクトリ) を削除しました。", repo_name.cyan());
    Ok(())
}

fn handle_repo_remote_command(command_opt: Option<RemoteRepoCommands>) -> Result<()> {
    let command = match command_opt {
        Some(cmd) => cmd,
        None => { // `mygit repo remote` のみでサブコマンドがない場合
            let choices = vec![
                SelectOption { display: "add: 新しいリモート'origin'を追加".to_string(), value: "add".to_string() },
                SelectOption { display: "set-url: 'origin'のURLを変更".to_string(), value: "set-url".to_string() },
                SelectOption { display: "remove: 'origin'を削除".to_string(), value: "remove".to_string() },
                SelectOption { display: "show: 現在の'origin'設定を表示".to_string(), value: "show".to_string() },
            ];
            match prompt_fuzzy_select("実行するリモート操作を選択:", &choices)? {
                Some(sub_cmd_str) => {
                    match sub_cmd_str.as_str() {
                        "add" => {
                            let url = prompt_non_empty_input("追加するリモート 'origin' のURL:", "URLは必須です。")?;
                            return git_repo_remote_add(url);
                        },
                        "set-url" => {
                            let url = prompt_non_empty_input("新しいリモート 'origin' のURL:", "URLは必須です。")?;
                            return git_repo_remote_set_url(url);
                        },
                        "remove" => return git_repo_remote_remove(),
                        "show" => return git_repo_remote_show(),
                        _ => bail!("不正な選択です。"),
                    }
                }
                None => {
                    println!("リモート操作がキャンセルされました。");
                    return Ok(());
                }
            }
        }
    };

    match command {
        RemoteRepoCommands::Add { url } => git_repo_remote_add(url),
        RemoteRepoCommands::SetUrl { url } => git_repo_remote_set_url(url),
        RemoteRepoCommands::Remove => git_repo_remote_remove(),
        RemoteRepoCommands::Show => git_repo_remote_show(),
    }
}

fn git_repo_remote_add(url: String) -> Result<()> {
    match GitCommand::remote_get_url("origin") {
        Ok(existing_url) if !existing_url.is_empty() => {
            bail!("エラー: リモート 'origin' は既に存在します (URL: {})。\n変更する場合は 'repo remote set-url' を、削除する場合は 'repo remote remove' を使用してください。", existing_url.cyan());
        }
        _ => {} // エラーまたはURLが空ならOK (新規追加可能)
    }
    GitCommand::remote_add("origin", &url).context(format!("リモート 'origin' の追加失敗 (URL: {})", url))?;
    println!("リモート 'origin' をURL '{}' で追加しました。", url.cyan());
    Ok(())
}

fn git_repo_remote_set_url(url: String) -> Result<()> {
    // 存在確認は set-url が内部で行うか、事前に get-url で確認
    match GitCommand::remote_get_url("origin") {
        Ok(_) => { // 存在する場合のみ set-url を許可
            GitCommand::remote_set_url("origin", &url).context(format!("リモート 'origin' のURL変更失敗 (URL: {})", url))?;
            println!("リモート 'origin' のURLを '{}' に変更しました。", url.cyan());
        }
        Err(_) => { // origin が存在しない場合
            bail!("エラー: リモート 'origin' が存在しません。\n追加する場合は 'repo remote add <url>' を使用してください。");
        }
    }
    Ok(())
}

fn git_repo_remote_remove() -> Result<()> {
    match GitCommand::remote_get_url("origin") {
        Ok(url) if !url.is_empty() => {
            if prompt_confirm(&format!("本当にリモート 'origin' (URL: {}) を削除しますか？", url.cyan()))? {
                GitCommand::remote_remove("origin").context("リモート 'origin' の削除失敗")?;
                println!("リモート 'origin' を削除しました。");
            } else {
                println!("リモート 'origin' の削除はキャンセルされました。");
            }
        }
        _ => { // URLが空か、get-urlがエラー
            println!("{}", "削除対象のリモート 'origin' は設定されていないか、取得できませんでした。".yellow());
        }
    }
    Ok(())
}

fn git_repo_remote_show() -> Result<()> {
    match GitCommand::remote_get_url("origin") {
        Ok(url) if !url.is_empty() => {
            println!("リモート 'origin' URL: {}", url.cyan());
        }
        _ => {
            println!("リモート 'origin' は設定されていないか、URLを取得できませんでした。");
        }
    }
    Ok(())
}