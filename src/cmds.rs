// cmds.rs

use std::process::exit;
use crate::{GitCommand, CommandDefinition};
use colored::*;
use crate::utils::{
    handle_command_result,
    get_current_branch_name,
    prompt_input,
    resolve_branch_input,
    get_branch_display_info,
    handle_conflict_and_offer_new_branch,
    GitCommandTrait,
    prompt_non_empty_input,
    prompt_select,
    ensure_branch_exists,
    get_origin_url,
    prompt_and_push_optional,
    get_branch_select_options,
};

// GitCommandの実装をGitCommandTraitに追加 (変更なし)
impl GitCommandTrait for GitCommand {
    fn symbolic_ref_head(&self) -> Result<String, String> {
        GitCommand::symbolic_ref_head()
    }
    fn rev_parse_verify(&self, ref_name: &str) -> Result<bool, String> {
        GitCommand::rev_parse_verify(ref_name)
    }
    fn rev_parse_commit_id(&self, ref_name: &str) -> Result<String, String> {
        GitCommand::rev_parse_commit_id(ref_name)
    }
    fn merge_base(&self, commit1: &str, commit2: &str) -> Result<String, String> {
        GitCommand::merge_base(commit1, commit2)
    }
    fn checkout_b(&self, branch: &str) -> Result<(), String> {
        GitCommand::checkout_b(branch)
    }
    fn remote_get_url(&self, remote: &str) -> Result<String, String> {
        GitCommand::remote_get_url(remote)
    }
    fn checkout(&self, branch: &str) -> Result<(), String> {
        GitCommand::checkout(branch)
    }
    fn push_u(&self, remote: &str, branch: &str) -> Result<(), String> {
        GitCommand::push_u(remote, branch)
    }
    fn branch_list_local_str(&self) -> Result<String, String> {
        GitCommand::branch_list_local_str()
    }
    fn branch_list_all_str(&self) -> Result<String, String> {
        GitCommand::branch_list_all_str()
    }
}

// print_usage_and_exit (変更なし)
pub fn print_usage_and_exit(program_name: &str, commands: &[CommandDefinition]) {
    eprintln!("{} {} {{command}} [args]", "Usage:".bold(), program_name.green());
    eprintln!("\n利用可能なコマンド:");
    for cmd_def in commands {
        eprintln!("  {:<10} {}", cmd_def.name.cyan(), cmd_def.description);
    }
    exit(1);
}

// show_help (変更なし)
pub fn show_help(args: &[String]) {
    let program_name = args.first().map_or("mygit", |s|s.as_str());
    println!("{} {} - Git操作を簡略化するCLIツール", program_name.bold().green(), std::env!("CARGO_PKG_VERSION"));
    println!("\n{} {}", "Usage:".bold(), program_name.green());
    println!("  {} {{save|setup|branch|switch|merge|copy|delete|create|help}}", program_name.green());
    println!("\n{} {}{}", "利用可能なコマンド:".bold(), "(詳細は ".dimmed(), "各コマンドのヘルプを参照ください (未実装)".dimmed());
    for cmd_def in crate::COMMAND_DEFINITIONS { // main.rs の COMMAND_DEFINITIONS を参照
        println!("  {:<10} {}", cmd_def.name.cyan(), cmd_def.description);
    }
    exit(0);
}

pub fn git_save(_args: &[String]) {
    handle_command_result(GitCommand::add("."), |_| {});
    let msg = prompt_non_empty_input("コミットメッセージ: ", "エラー: メッセージ必須。");
    handle_command_result(GitCommand::commit(&msg), |_| {});
    println!("ローカルにコミットしました。");

    let current_branch = get_current_branch_name(&GitCommand);
    if current_branch.is_empty() {
        eprintln!("{}", "エラー: 現在のブランチ不明。プッシュをスキップ。".yellow());
        return;
    }

    let remote_url = get_origin_url(&GitCommand);
    if !remote_url.is_empty() {
        let push_prompt = format!("リモート 'origin/{}' にもプッシュしますか？ (y/N): ", current_branch);
        if prompt_input(&push_prompt).eq_ignore_ascii_case("y") {
            handle_command_result(GitCommand::push_u("origin", &current_branch), |_| {});
            println!("'origin/{}' へプッシュしました。", current_branch.cyan());
            
            if prompt_input("リモートの最新の変更をプルしますか？ (コンフリクトの可能性あり) (y/N): ").eq_ignore_ascii_case("y") {
                let mut pull_success = false;
                handle_command_result(GitCommand::pull("origin", &current_branch), |success| pull_success = success);
                if pull_success {
                    println!("{}", "プル成功。最新の状態です。".green());
                } else {
                    handle_conflict_and_offer_new_branch("プル", &GitCommand);
                }
            }
        } else {
            println!("リモートへのプッシュはスキップしました。");
        }
    } else {
        println!("{}", "リモート 'origin' が未設定のため、プッシュはスキップしました。".yellow());
    }
    println!("{}", "保存処理が完了しました。".green());
}

pub fn git_setup(_args: &[String]) {
    if !std::path::Path::new(".git").exists() {
        handle_command_result(GitCommand::init(), |_| {});
        println!("Gitリポジトリを初期化しました。");
    }
    
    let current_url = get_origin_url(&GitCommand);
    if !current_url.is_empty() {
        println!("現在のリモート 'origin' URL: {}", current_url.cyan());
    } else {
        println!("リモート 'origin' は現在設定されていません。");
    }
    
    // utils::SelectOption を使う場合。もし utils::SelectOption が String のみ受け付けるなら調整必要
    // ここでは value を &str にしているため、utils::SelectOption<String> を使うか、
    // utils::prompt_select の value の型パラメータを調整する必要がある。
    // 簡単のため、ここでは PromptuitySelectOption を直接使う元の形に近い形で残すが、
    // prompt_select ヘルパーを使うなら、その定義に合わせる。
    // 仮に utils::prompt_select が Vec<utils::SelectOption<&str>> を取ると仮定。
    // pub struct SelectOption<T: Clone> { pub display: String, pub value: T } を utils に定義したとする。
    let options = vec![
        crate::utils::SelectOption { display: "URLを新規追加/変更する".to_string(), value: "add_or_set_url" },
        crate::utils::SelectOption { display: "リモート 'origin' を削除する".to_string(), value: "remove_url" },
        crate::utils::SelectOption { display: "今回は何もしない".to_string(), value: "cancel" },
    ];
    let selected_action = prompt_select("リモート 'origin' に対する操作を選択してください:", options);

    match selected_action { // selected_action は &str (または prompt_select の返り値型)
        "add_or_set_url" => {
            let new_url = prompt_input("新しいリモート 'origin' のURLを入力してください: ");
            if new_url.is_empty() {
                println!("URLが入力されなかったので、設定/変更は行いませんでした。");
            } else {
                if current_url.is_empty() {
                    handle_command_result(GitCommand::remote_add("origin", &new_url), |_| {});
                    println!("リモート 'origin' をURL '{}' で追加しました。", new_url.cyan());
                } else if current_url != new_url {
                    handle_command_result(GitCommand::remote_set_url("origin", &new_url), |_| {});
                    println!("リモート 'origin' のURLを '{}' に変更しました。", new_url.cyan());
                } else {
                    println!("入力されたURLは現在の設定と同じです。変更はありません。");
                }
            }
        }
        "remove_url" => {
            if !current_url.is_empty() {
                if prompt_input("本当にリモート 'origin' を削除 (追跡を解除) しますか？ (y/N): ").eq_ignore_ascii_case("y") {
                    handle_command_result(GitCommand::remote_remove("origin"), |_| {});
                    println!("リモート 'origin' を削除しました。");
                } else {
                    println!("リモート 'origin' の削除はキャンセルされました。");
                }
            } else {
                println!("削除するリモート 'origin' は設定されていません。");
            }
        }
        _ => { // "cancel" または予期せぬ値
            println!("リモート 'origin' に関する操作は行いませんでした。");
        }
    }
    println!("{}", "セットアップ処理を終了します。".green());
}

pub fn git_branch(_args: &[String]) {
    let remote_url = get_origin_url(&GitCommand);

    if !remote_url.is_empty() {
        handle_command_result(GitCommand::fetch_prune("origin"), |_| {});
        println!("ブランチ一覧 (リモート 'origin' を含む):");
    } else {
        println!("ローカルブランチ一覧 (リモート 'origin' 未設定):");
    }

    let mut branches_all_str = String::new();
    handle_command_result(GitCommand::branch_list_all_str(), |s| branches_all_str = s);
    
    let mut uncommitted_changes = false;
    handle_command_result(GitCommand::status_porcelain_v1(), |s| uncommitted_changes = !s.is_empty());

    let mut displayed_locals = std::collections::HashSet::new();

    for line in branches_all_str.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() || trimmed_line.contains(" -> ") || trimmed_line.ends_with("/HEAD") { // スキップ条件を統合
            continue;
        }

        let is_current = trimmed_line.starts_with("* ");
        let name_part = trimmed_line.trim_start_matches("* ");
        
        // "remotes/origin/branch" or "branch"
        let (display_name, is_remote_line) = if let Some(remote_branch_name) = name_part.strip_prefix("remotes/origin/") {
            (remote_branch_name.to_string(), true)
        } else {
            (name_part.to_string(), false)
        };
        
        if is_remote_line {
            if !displayed_locals.contains(&display_name) {
                 println!("  {} {}", display_name.blue(), "(リモートのみ)".dimmed());
            }
        } else { // Local branch
            displayed_locals.insert(display_name.clone());
            let (display_str, note) = get_branch_display_info(&display_name, is_current, uncommitted_changes, &remote_url, &GitCommand);
            println!("{} {}", display_str, note);
        }
    }
}

pub fn git_switch(_args: &[String]) {
    let branch_options = get_branch_select_options(&GitCommand);
    if branch_options.is_empty() {
        println!("{}", "利用可能なブランチがありません。".yellow());
        return;
    }

    println!("{}", "ブランチを選択してください:".yellow());
    let target_input = prompt_select("切り替えたいブランチ:", branch_options);

    let resolved = resolve_branch_input(&target_input, &GitCommand);

    if resolved.local_exists && resolved.local_candidate_name == target_input {
        handle_command_result(GitCommand::checkout(&resolved.local_candidate_name), |_| {});
        println!("ブランチ '{}' へ切り替えました。", resolved.local_candidate_name.cyan());
    } else if resolved.remote_tracking_exists {
        let prompt_msg = format!(
            "ローカルブランチ '{}' をリモートブランチ '{}' から作成して切り替えますか？ (y/N): ",
            resolved.local_candidate_name.yellow(), resolved.remote_tracking_candidate_name.blue()
        );
        if prompt_input(&prompt_msg).eq_ignore_ascii_case("y") {
            handle_command_result(GitCommand::checkout(&resolved.local_candidate_name), |_| {});
            println!("ブランチ '{}' を作成し、リモート '{}' を追跡して切り替えました。",
                    resolved.local_candidate_name.cyan(), resolved.remote_tracking_candidate_name.blue());
        } else {
            println!("操作をキャンセルしました。");
        }
    } else {
        eprintln!("エラー: 選択されたブランチ '{}' が見つかりません。", target_input.red());
        exit(1);
    }
}

pub fn git_merge(_args: &[String]) {
    let cur_b = get_current_branch_name(&GitCommand);
    if cur_b.is_empty() { eprintln!("{}", "エラー: 現在のブランチ不明。".red()); exit(1); }

    let target_prompt = format!("ブランチ '{}' にマージするブランチ名: ", cur_b.cyan());
    let target = prompt_non_empty_input(&target_prompt, "エラー: マージ対象名必須。");
    ensure_branch_exists(&target, &GitCommand, ""); // "" verb for generic "branch"
    
    let mut merge_success = false;
    handle_command_result(GitCommand::merge(&target), |success| merge_success = success);

    if merge_success {
        println!("{}", "マージ成功。".green());
        let delete_prompt = format!("マージ元のローカルブランチ '{}' を削除しますか？ (y/N): ", target);
        if prompt_input(&delete_prompt).eq_ignore_ascii_case("y") {
            handle_command_result(GitCommand::branch_delete_local_d(&target), |_| {}); 
            println!("ローカルブランチ '{}' を削除しました。", target.cyan());
        }
    } else {
        handle_conflict_and_offer_new_branch("マージ", &GitCommand);
    }
}

pub fn git_copy(_args: &[String]) {
    let source = prompt_non_empty_input("コピー元ブランチ名: ", "エラー: コピー元ブランチ名必須。");
    ensure_branch_exists(&source, &GitCommand, "コピー元");

    let new_name = prompt_non_empty_input("新しいブランチ名: ", "エラー: 新ブランチ名必須。");
    let mut new_exists = false; // Check if new branch name already exists
    handle_command_result(GitCommand::rev_parse_verify(&new_name), |e| new_exists = e);
    if new_exists { eprintln!("エラー: ブランチ '{}' は既に存在。", new_name.red()); exit(1); }

    handle_command_result(GitCommand::branch_create_local_from(&new_name, &source), |_| {});
    println!("ローカルブランチ '{}' を '{}' からコピーしました。", new_name.cyan(), source.cyan());

    prompt_and_push_optional(&new_name, "コピー", &GitCommand, true); // needs_checkout = true
}

pub fn git_delete(_args: &[String]) {
    let branch_options = get_branch_select_options(&GitCommand);
    if branch_options.is_empty() {
        println!("{}", "削除可能なブランチがありません。".yellow());
        return;
    }

    println!("{}", "ブランチを選択してください:".yellow());
    let target_input = prompt_select("削除するブランチ:", branch_options);

    let resolved = resolve_branch_input(&target_input, &GitCommand);
    let current_branch_name = get_current_branch_name(&GitCommand);
    let remote_url = get_origin_url(&GitCommand);
    let mut something_done = false;

    if resolved.local_exists {
        if current_branch_name == resolved.local_candidate_name {
            eprintln!("エラー: 現在チェックアウト中のローカルブランチ '{}' は削除できません。", resolved.local_candidate_name.red());
        } else {
            let local_delete_prompt = format!("ローカルブランチ '{}' を削除しますか？ (y/N): ", resolved.local_candidate_name.truecolor(255,165,0));
            if prompt_input(&local_delete_prompt).eq_ignore_ascii_case("y") {
                handle_command_result(GitCommand::branch_delete_local_d(&resolved.local_candidate_name), |_| {});
                println!("ローカルブランチ '{}' を削除しました。", resolved.local_candidate_name.truecolor(255,165,0));
                something_done = true;
            }
        }
    } else if !target_input.starts_with("origin/") {
        println!("情報: ローカルブランチ '{}' は見つかりませんでした（選択肢にはあったはずですが）。", resolved.local_candidate_name.yellow());
    }

    if resolved.remote_tracking_exists && !remote_url.is_empty() {
        let remote_delete_prompt = format!("リモートブランチ '{}' を削除しますか？ (y/N): ", resolved.remote_tracking_candidate_name.blue());
        if prompt_input(&remote_delete_prompt).eq_ignore_ascii_case("y") {
            handle_command_result(GitCommand::push_delete("origin", &resolved.local_candidate_name), |_| {});
            println!("リモートブランチ '{}' の削除を試みました。", resolved.remote_tracking_candidate_name.blue());
            something_done = true;
        }
    } else if target_input.starts_with("origin/") && !remote_url.is_empty() && !resolved.remote_tracking_exists {
        println!("情報: リモートブランチ '{}' は見つかりませんでした（選択肢にはあったはずですが）。", resolved.remote_tracking_candidate_name.yellow());
    }

    if !something_done {
        println!("ブランチ削除は行われませんでした。");
    }
}

pub fn git_create(_args: &[String]) {
    let name = prompt_non_empty_input("作成する新しいローカルブランチ名: ", "エラー: ブランチ名必須。");
    let mut exists = false;
    handle_command_result(GitCommand::rev_parse_verify(&name), |e| exists = e);
    if exists { eprintln!("エラー: ブランチ '{}' は既にローカルに存在します。", name.red()); exit(1); }

    handle_command_result(GitCommand::branch_create_local(&name), |_| {});
    println!("ローカルブランチ '{}' を作成しました。", name.truecolor(255,165,0));
    
    prompt_and_push_optional(&name, "作成", &GitCommand, true); // needs_checkout = true
}

pub fn git_tree(_args: &[String]) {
    handle_command_result(GitCommand::show_branch_tree(), |output| {
        println!("{}", output);
    });
}