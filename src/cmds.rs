// cmds.rs

use std::process::exit;
use crate::{GitCommand, CommandDefinition, CommandResult}; // main.rs からインポート
use colored::*; // colored の Colorize トレイトをインポート
use promptuity::prompts::{Input, Select, SelectOption};
use promptuity::themes::MinimalTheme;
use promptuity::{Promptuity, Term};

// エラー処理を一箇所にまとめるヘルパー
fn handle_command_result<T, F>(result: CommandResult<T>, success_action: F)
where
    F: FnOnce(T),
{
    match result {
        Ok(val) => success_action(val),
        Err(err_msg) => {
            eprintln!("{}", err_msg.red()); // .red() を使用
            exit(1);
        }
    }
}
fn handle_command_result_void(result: CommandResult<()>) {
    handle_command_result(result, |_| {});
}

// CommandHandler の型エイリアスは main.rs で pub として定義されているので、ここでは不要
// pub type CommandHandler = fn(&[String]);


pub fn print_usage_and_exit(program_name: &str, commands: &[CommandDefinition]) {
    eprintln!("{} {} {{command}} [args]", "Usage:".bold(), program_name.green());
    eprintln!("\n利用可能なコマンド:");
    for cmd_def in commands {
        eprintln!("  {:<10} {}", cmd_def.name.cyan(), cmd_def.description);
    }
    exit(1);
}

pub fn show_help(args: &[String]) {
    let program_name = args.first().map_or("mygit", |s|s.as_str());
    // CARGO_PKG_VERSION を使うには std::env を use する必要がある
    println!("{} {} - Git操作を簡略化するCLIツール", program_name.bold().green(), std::env!("CARGO_PKG_VERSION"));
    println!("\n{} {}", "Usage:".bold(), program_name.green());
    println!("  {} {{save|setup|branch|switch|merge|copy|delete|create|help}}", program_name.green());
    println!("\n{} {}{}", "利用可能なコマンド:".bold(), "(詳細は ".dimmed(), "各コマンドのヘルプを参照ください (未実装)".dimmed());
    for cmd_def in crate::COMMAND_DEFINITIONS {
        println!("  {:<10} {}", cmd_def.name.cyan(), cmd_def.description);
    }
    exit(0);
}


fn get_current_branch_name() -> String {
    let mut current_branch = String::new();
    handle_command_result(GitCommand::symbolic_ref_head(), |s| current_branch = s);
    current_branch
}

fn prompt_input(message: &str) -> String {
    let mut term = Term::default();
    let mut theme = MinimalTheme::default();
    let mut p = Promptuity::new(&mut term, &mut theme);
    let mut input_prompt = Input::new(message.to_string());
    if let Err(e) = p.begin() { eprintln!("エラー: プロンプト初期化 ({:?})", e); exit(1); }
    let result = match p.prompt(&mut input_prompt) {
        Ok(res) => res,
        Err(e) => { eprintln!("エラー: 入力取得 ({:?})", e); exit(1); }
    };
    if let Err(e) = p.finish() { eprintln!("警告: プロンプト終了処理 ({:?})", e); }
    result
}

fn confirm(message: &str) -> bool {
    prompt_input(&format!("{} (y/N): ", message)).eq_ignore_ascii_case("y")
}

fn handle_conflict_and_offer_new_branch(operation_name: &str, _current_branch_for_checkout_b: &str) {
    eprintln!("警告: {} に失敗しました。コンフリクトの可能性があります。", operation_name.yellow());
    if confirm("この状態で新しいブランチを作成して変更を保持しますか？") {
        let new_branch_name = prompt_input("新しいブランチ名: ");
        if new_branch_name.is_empty() {
            eprintln!("{}", "エラー: ブランチ名が入力されませんでした。".red());
        } else {
            let mut already_exists = false;
            handle_command_result(GitCommand::rev_parse_verify(&new_branch_name), |exists| already_exists = exists);
            if already_exists { eprintln!("エラー: ブランチ '{}' は既に存在します。", new_branch_name.bold().red()); exit(1); }
            
            handle_command_result_void(GitCommand::checkout_b(&new_branch_name));
            println!("新しいブランチ '{}' を作成し切り替えました。", new_branch_name.cyan());
            println!("コンフリクトを解決し、再度 {} を試みてください。", operation_name.yellow());
            exit(0); 
        }
    }
    println!("新しいブランチは作成しませんでした。手動で状況を確認してください。");
    exit(1);
}


pub fn git_save(_args: &[String]) {
    handle_command_result_void(GitCommand::add("."));
    let msg = prompt_input("コミットメッセージ: ");
    if msg.is_empty() { eprintln!("{}", "エラー: メッセージ必須。".red()); exit(1); }
    handle_command_result_void(GitCommand::commit(&msg));
    println!("ローカルにコミットしました。");

    let current_branch = get_current_branch_name();
    if current_branch.is_empty() { eprintln!("{}", "エラー: 現在のブランチ不明。プッシュをスキップ。".yellow()); return; }

    let mut remote_url = String::new();
    // remote_get_url は失敗する可能性があるので、エラーハンドリングする
    match GitCommand::remote_get_url("origin") {
        Ok(url) => remote_url = url,
        Err(_) => { /* origin がなければ空のまま */ }
    }


    if !remote_url.is_empty() {
        if confirm(&format!("リモート 'origin/{}' にもプッシュしますか？", current_branch)) {
            handle_command_result_void(GitCommand::push_u("origin", &current_branch));
            println!("'origin/{}' へプッシュしました。", current_branch.cyan());
            if confirm("リモートの最新の変更をプルしますか？ (コンフリクトの可能性あり)") {
                let mut pull_success = false;
                handle_command_result(GitCommand::pull("origin", &current_branch), |success| pull_success = success);
                if pull_success {
                    println!("{}", "プル成功。最新の状態です。".green());
                } else {
                    handle_conflict_and_offer_new_branch("プル", &current_branch);
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
        handle_command_result_void(GitCommand::init());
        println!("Gitリポジトリを初期化しました。");
    }
    
    let mut current_url_opt: Option<String> = None;
    match GitCommand::remote_get_url("origin") {
        Ok(url) if !url.is_empty() => {
            println!("現在のリモート 'origin' URL: {}", url.cyan());
            current_url_opt = Some(url);
        }
        _ => println!("リモート 'origin' は現在設定されていません。"),
    }

    // ユーザーに操作を選択させる
    let mut term = Term::default();
    let mut theme = MinimalTheme::default(); // または好みのテーマ
    let mut p = Promptuity::new(&mut term, &mut theme);

    let options = vec![
        SelectOption::new("URLを新規追加/変更する", "add_or_set_url"),
        SelectOption::new("リモート 'origin' を削除する", "remove_url"),
        SelectOption::new("今回は何もしない", "cancel"),
    ];
    
    let mut select_prompt = Select::new("リモート 'origin' に対する操作を選択してください:", options);

    if let Err(e) = p.begin() { eprintln!("エラー: プロンプト初期化 ({:?})", e); exit(1); }
    let selected_action = match p.prompt(&mut select_prompt) {
        Ok(action_id) => action_id,
        Err(e) => { eprintln!("エラー: 選択肢の取得 ({:?})", e); exit(1); }
    };
    if let Err(e) = p.finish() { eprintln!("警告: プロンプト終了処理 ({:?})", e); }


    match selected_action.as_ref() {
        "add_or_set_url" => {
            let new_url = prompt_input("新しいリモート 'origin' のURLを入力してください: ");
            if new_url.is_empty() {
                println!("URLが入力されなかったので、設定/変更は行いませんでした。");
            } else {
                if current_url_opt.is_none() { // 現在リモートがない場合は追加
                    handle_command_result_void(GitCommand::remote_add("origin", &new_url));
                    println!("リモート 'origin' をURL '{}' で追加しました。", new_url.cyan());
                } else if current_url_opt.as_deref() != Some(&new_url) { // 現在と異なるURLが入力された場合は変更
                    handle_command_result_void(GitCommand::remote_set_url("origin", &new_url));
                    println!("リモート 'origin' のURLを '{}' に変更しました。", new_url.cyan());
                } else { // 現在と同じURLが入力された場合
                    println!("入力されたURLは現在の設定と同じです。変更はありません。");
                }
            }
        }
        "remove_url" => {
            if current_url_opt.is_some() {
                if confirm("本当にリモート 'origin' を削除 (追跡を解除) しますか？") {
                    handle_command_result_void(GitCommand::remote_remove("origin"));
                    println!("リモート 'origin' を削除しました。");
                } else {
                    println!("リモート 'origin' の削除はキャンセルされました。");
                }
            } else {
                println!("削除するリモート 'origin' は設定されていません。");
            }
        }
        "cancel" | _ => { // "cancel" または予期せぬ値
            println!("リモート 'origin' に関する操作は行いませんでした。");
        }
    }
    println!("{}", "セットアップ処理を終了します。".green());
}


#[derive(PartialEq, Debug)]
enum BranchDisplayStatus { Synced, LocalOnly, Ahead, Behind, Diverged }

fn get_branch_display_status(local_branch: &str, local_id: &str) -> (BranchDisplayStatus, String) {
    let remote_tracking_branch = format!("origin/{}", local_branch);
    let mut note = String::new();

    let remote_id_res = GitCommand::rev_parse_verify(&remote_tracking_branch)
        .and_then(|exists| if exists { GitCommand::rev_parse_commit_id(&remote_tracking_branch)} else { Ok(String::new()) });

    let status = match remote_id_res {
        Ok(remote_id) if !remote_id.is_empty() => {
            if local_id == remote_id {
                BranchDisplayStatus::Synced
            } else {
                match GitCommand::merge_base(local_id, &remote_id) {
                    Ok(base_id) => {
                        if base_id == remote_id { note = "(要プッシュ)".dimmed().to_string(); BranchDisplayStatus::Ahead }
                        else if base_id == local_id { note = "(要プル)".dimmed().to_string(); BranchDisplayStatus::Behind }
                        else { note = "(分岐)".dimmed().to_string(); BranchDisplayStatus::Diverged }
                    }
                    Err(_) => BranchDisplayStatus::LocalOnly, // merge-base失敗は判定不能->LocalOnly
                }
            }
        }
        _ => BranchDisplayStatus::LocalOnly,
    };
    (status, note)
}


pub fn git_branch(_args: &[String]) {
    let mut remote_url = String::new();
    handle_command_result(GitCommand::remote_get_url("origin"), |url| remote_url = url);

    if !remote_url.is_empty() {
        handle_command_result_void(GitCommand::fetch_prune("origin"));
        println!("ブランチ一覧 (リモート 'origin' を含む):");
    } else {
        println!("ローカルブランチ一覧 (リモート 'origin' 未設定):");
    }

    let mut branches_all_str = String::new();
    handle_command_result(GitCommand::branch_list_all_str(), |s| branches_all_str = s);
    
    let _current_branch_name = get_current_branch_name();
    let mut uncommitted_changes = false;
    handle_command_result(GitCommand::status_porcelain_v1(), |s| uncommitted_changes = !s.is_empty());

    let mut displayed_locals = std::collections::HashSet::new();

    for line in branches_all_str.lines() {
        let trimmed_line = line.trim();
        let is_current = trimmed_line.starts_with("* ");
        let branch_name_raw = trimmed_line.trim_start_matches("* ").trim_start_matches("remotes/");
        
        if branch_name_raw.is_empty() || branch_name_raw.ends_with("/HEAD") || branch_name_raw.contains("->") { continue; }

        let display_name = if branch_name_raw.starts_with("origin/") {
            branch_name_raw.trim_start_matches("origin/").to_string()
        } else {
            branch_name_raw.to_string()
        };

        if trimmed_line.starts_with("remotes/origin/") {
            if !displayed_locals.contains(&display_name) {
                 println!("  {} {}", display_name.blue(), "(リモートのみ)".dimmed());
            }
        } else {
            displayed_locals.insert(display_name.clone());
            let mut local_id = String::new();
            handle_command_result(GitCommand::rev_parse_commit_id(&display_name), |id| local_id = id);

            let (status, note) = if !remote_url.is_empty() && !local_id.is_empty() {
                get_branch_display_status(&display_name, &local_id)
            } else {
                (BranchDisplayStatus::LocalOnly, String::new())
            };
            
            let display_str = match status {
                BranchDisplayStatus::Synced => format!("  {}", display_name.blue()),
                BranchDisplayStatus::LocalOnly | BranchDisplayStatus::Ahead | BranchDisplayStatus::Behind | BranchDisplayStatus::Diverged => {
                    format!("  {}", display_name.truecolor(255,165,0)) // オレンジ (colored)
                }
            };
            if is_current {
                println!("* {} {}", display_name.cyan().bold(), if uncommitted_changes { "*".yellow().bold() } else { "".normal() });
            } else {
                println!("{} {}", display_str, note);
            }
        }
    }
}


pub fn git_switch(_args: &[String]) {
    println!("ローカルブランチ一覧:"); 
    let mut branches_str = String::new();
    handle_command_result(GitCommand::branch_list_local_str(), |s| branches_str = s);
    branches_str.lines().for_each(|l| if !l.trim().is_empty() {
        let current = l.starts_with("* ");
        let name = l.trim_start_matches("* ").trim();
        if current { println!("* {}", name.cyan().bold()); } 
        else { println!("  {}", name.truecolor(255,165,0)); } // オレンジ
    });

    let name = prompt_input("切り替えるブランチ名: ");
    if name.is_empty() { eprintln!("{}", "エラー: ブランチ名必須。".red()); exit(1); }
    let mut exists = false;
    handle_command_result(GitCommand::rev_parse_verify(&name), |e| exists = e);
    if !exists { eprintln!("エラー: ブランチ '{}' はローカルに存在せず。", name.red()); exit(1); }
    
    handle_command_result_void(GitCommand::checkout(&name)); 
    println!("ブランチ '{}' へ切り替えました。", name.cyan());
}

pub fn git_merge(_args: &[String]) {
    let cur_b = get_current_branch_name();
    if cur_b.is_empty() { eprintln!("{}", "エラー: 現在のブランチ不明。".red()); exit(1); }
    let target = prompt_input(&format!("ブランチ '{}' にマージするブランチ名: ", cur_b.cyan()));
    if target.is_empty() { eprintln!("{}", "エラー: マージ対象名必須。".red()); exit(1); }
    let mut target_exists = false;
    handle_command_result(GitCommand::rev_parse_verify(&target), |e| target_exists = e);
    if !target_exists { eprintln!("エラー: ブランチ '{}' は存在せず。", target.red()); exit(1); }
    
    let mut merge_success = false;
    handle_command_result(GitCommand::merge(&target), |success| merge_success = success);

    if merge_success {
        println!("{}", "マージ成功。".green());
        if confirm(&format!("マージ元のローカルブランチ '{}' を削除しますか？", target)) {
            handle_command_result_void(GitCommand::branch_delete_local_d(&target)); 
            println!("ローカルブランチ '{}' を削除しました。", target.cyan());
        }
    } else {
        handle_conflict_and_offer_new_branch("マージ", &cur_b);
    }
}

pub fn git_copy(_args: &[String]) {
    let source = prompt_input("コピー元ブランチ名: ");
    if source.is_empty() { eprintln!("{}", "エラー: コピー元ブランチ名必須。".red()); exit(1); }
    let mut source_exists = false;
    handle_command_result(GitCommand::rev_parse_verify(&source), |e| source_exists = e);
    if !source_exists { eprintln!("エラー: コピー元ブランチ '{}' が無効。", source.red()); exit(1); }

    let new_name = prompt_input("新しいブランチ名: ");
    if new_name.is_empty() { eprintln!("{}", "エラー: 新ブランチ名必須。".red()); exit(1); }
    let mut new_exists = false;
    handle_command_result(GitCommand::rev_parse_verify(&new_name), |e| new_exists = e);
    if new_exists { eprintln!("エラー: ブランチ '{}' は既に存在。", new_name.red()); exit(1); }

    handle_command_result_void(GitCommand::branch_create_local_from(&new_name, &source));
    println!("ローカルブランチ '{}' を '{}' からコピーしました。", new_name.cyan(), source.cyan());

    let mut remote_url = String::new();
    handle_command_result(GitCommand::remote_get_url("origin"), |url| remote_url = url);
    if !remote_url.is_empty() && confirm(&format!("コピーしたブランチ '{}' をリモート 'origin' にプッシュし追跡設定しますか？", new_name)) {
        handle_command_result_void(GitCommand::checkout(&new_name)); 
        handle_command_result_void(GitCommand::push_u("origin", &new_name)); 
        println!("ブランチ '{}' を 'origin/{}' へプッシュし追跡設定しました。", new_name.cyan(), new_name.blue());
    }
}

pub fn git_delete(_args: &[String]) {
    let mut remote_url = String::new();
    handle_command_result(GitCommand::remote_get_url("origin"), |url| remote_url = url);
    if !remote_url.is_empty() { handle_command_result_void(GitCommand::fetch_prune("origin")); }

    println!("現在のブランチ (ローカルとリモート origin):");
    git_branch(&[]); 

    let name_input = prompt_input("削除するブランチ名 (ローカル名 or origin/リモート名): ");
    if name_input.is_empty() { eprintln!("{}", "エラー: 削除ブランチ名必須。".red()); exit(1); }

    let current_branch = get_current_branch_name();
    if current_branch == name_input {
        eprintln!("エラー: 現在チェックアウト中のローカルブランチ '{}' は削除できません。", name_input.red());
        exit(1);
    }

    if name_input.starts_with("origin/") {
        if remote_url.is_empty() { eprintln!("{}", "エラー: リモート 'origin' が未設定。".red()); exit(1); }
        let remote_branch_name = name_input.trim_start_matches("origin/");
        if confirm(&format!("リモートブランチ 'origin/{}' を削除しますか？", remote_branch_name)) {
            handle_command_result_void(GitCommand::push_delete("origin", remote_branch_name));
            println!("リモートブランチ 'origin/{}' の削除を試みました。", remote_branch_name.blue());
        }
    } else {
        let mut local_exists = false;
        handle_command_result(GitCommand::rev_parse_verify(&name_input), |e| local_exists = e);
        if local_exists {
            if confirm(&format!("ローカルブランチ '{}' を削除しますか？", name_input)) {
                handle_command_result_void(GitCommand::branch_delete_local_d(&name_input));
                println!("ローカルブランチ '{}' を削除しました。", name_input.truecolor(255,165,0)); // オレンジ
            }
        } else {
            println!("ローカルブランチ '{}' は見つかりませんでした。", name_input.yellow());
        }
        if !remote_url.is_empty() && confirm(&format!("(もし存在すれば) リモートブランチ 'origin/{}' も削除しますか？", name_input)) {
             handle_command_result_void(GitCommand::push_delete("origin", &name_input));
             println!("リモートブランチ 'origin/{}' の削除を試みました。", name_input.blue());
        }
    }
}

pub fn git_create(_args: &[String]) {
    let name = prompt_input("作成する新しいローカルブランチ名: ");
    if name.is_empty() { eprintln!("{}", "エラー: ブランチ名必須。".red()); exit(1); }
    let mut exists = false;
    handle_command_result(GitCommand::rev_parse_verify(&name), |e| exists = e);
    if exists { eprintln!("エラー: ブランチ '{}' は既にローカルに存在します。", name.red()); exit(1); }
    
    handle_command_result_void(GitCommand::branch_create_local(&name));
    println!("ローカルブランチ '{}' を作成しました。", name.truecolor(255,165,0)); // オレンジ

    let mut remote_url = String::new();
    handle_command_result(GitCommand::remote_get_url("origin"), |url| remote_url = url);
    if !remote_url.is_empty() && confirm(&format!("作成したブランチ '{}' をリモート 'origin' にプッシュし追跡設定しますか？", name)) {
        handle_command_result_void(GitCommand::checkout(&name));
        handle_command_result_void(GitCommand::push_u("origin", &name));
        println!("ブランチ '{}' を 'origin/{}' へプッシュし追跡設定しました。", name.cyan(), name.blue());
    }
}

pub fn git_tree(_args: &[String]) {
    handle_command_result(GitCommand::show_branch_tree(), |output| {
        println!("{}", output);
    });
}