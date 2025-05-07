use std::process::exit;
use colored::*;
use promptuity::prompts::{Input, Select, SelectOption as PromptuitySelectOption};
use promptuity::themes::MinimalTheme;
use promptuity::{Promptuity, Term};

// エラー処理を一箇所にまとめるヘルパー
pub fn handle_command_result<T, F>(result: Result<T, String>, success_action: F)
where
    F: FnOnce(T),
{
    match result {
        Ok(val) => success_action(val),
        Err(err_msg) => {
            eprintln!("{}", err_msg.red());
            exit(1);
        }
    }
}

// handle_command_result_void は削除 (呼び出し側で handle_command_result(result, |_| {}) を使用)

// confirm は削除 (呼び出し側で prompt_input(...).eq_ignore_ascii_case("y") を使用)

// 現在のブランチ名を取得
pub fn get_current_branch_name(git_command: &dyn GitCommandTrait) -> String {
    let mut current_branch = String::new();
    handle_command_result(git_command.symbolic_ref_head(), |s| current_branch = s);
    current_branch
}

// ユーザー入力のプロンプト
pub fn prompt_input(message: &str) -> String {
    let mut term = Term::default();
    let mut theme = MinimalTheme::default();
    let mut p = Promptuity::new(&mut term, &mut theme);

    p.begin().unwrap_or_else(|e| {
        eprintln!("エラー: プロンプト初期化 ({:?})", e);
        exit(1);
    });

    let result = p.prompt(&mut Input::new(message.to_string()))
        .unwrap_or_else(|e| {
            eprintln!("エラー: 入力取得 ({:?})", e);
            exit(1);
        });

    if let Err(e) = p.finish() {
        eprintln!("警告: プロンプト終了処理 ({:?})", e);
    }
    result
}

// ブランチ名の解決情報を保持する構造体
pub struct ResolvedBranchInfo {
    pub local_candidate_name: String,
    pub local_exists: bool,
    pub remote_tracking_candidate_name: String,
    pub remote_tracking_exists: bool,
}

// Gitコマンドのトレイト定義
pub trait GitCommandTrait {
    fn symbolic_ref_head(&self) -> Result<String, String>;
    fn rev_parse_verify(&self, ref_name: &str) -> Result<bool, String>;
    fn rev_parse_commit_id(&self, ref_name: &str) -> Result<String, String>;
    fn merge_base(&self, commit1: &str, commit2: &str) -> Result<String, String>;
    fn checkout_b(&self, branch: &str) -> Result<(), String>;
    fn remote_get_url(&self, remote: &str) -> Result<String, String>; // cmds.rs で使用想定
    fn checkout(&self, branch: &str) -> Result<(), String>;          // cmds.rs で使用想定
    fn push_u(&self, remote: &str, branch: &str) -> Result<(), String>; // cmds.rs で使用想定
    fn branch_list_local_str(&self) -> Result<String, String>;
    fn branch_list_all_str(&self) -> Result<String, String>;
}

// ブランチ名を解決するヘルパー関数
pub fn resolve_branch_input(name_from_user: &str, git_command: &dyn GitCommandTrait) -> ResolvedBranchInfo {
    let local_candidate_name: String;
    let remote_tracking_candidate_name: String;

    if name_from_user.starts_with("origin/") {
        local_candidate_name = name_from_user.trim_start_matches("origin/").to_string();
        remote_tracking_candidate_name = name_from_user.to_string();
    } else {
        local_candidate_name = name_from_user.to_string();
        remote_tracking_candidate_name = format!("origin/{}", name_from_user);
    }

    let mut local_exists = false;
    if !name_from_user.starts_with("origin/") {
        handle_command_result(git_command.rev_parse_verify(&local_candidate_name), |e| local_exists = e);
    }
    
    let mut remote_tracking_exists = false;
    handle_command_result(git_command.rev_parse_verify(&remote_tracking_candidate_name), |e| remote_tracking_exists = e);

    ResolvedBranchInfo {
        local_candidate_name,
        local_exists,
        remote_tracking_candidate_name,
        remote_tracking_exists,
    }
}

// ブランチの表示状態を表す列挙型
#[derive(PartialEq, Debug)]
pub enum BranchDisplayStatus { Synced, LocalOnly, Ahead, Behind, Diverged }

// ブランチの表示状態を取得する関数
pub fn get_branch_display_status(local_branch: &str, local_id: &str, git_command: &dyn GitCommandTrait) -> (BranchDisplayStatus, String) {
    let remote_tracking_branch = format!("origin/{}", local_branch);

    match git_command.rev_parse_verify(&remote_tracking_branch)
        .and_then(|exists| if exists { git_command.rev_parse_commit_id(&remote_tracking_branch)} else { Ok(String::new()) }) {
        Ok(remote_id) if !remote_id.is_empty() => {
            if local_id == remote_id {
                (BranchDisplayStatus::Synced, String::new())
            } else {
                match git_command.merge_base(local_id, &remote_id) {
                    Ok(base_id) => {
                        if base_id == remote_id { (BranchDisplayStatus::Ahead, "(要プッシュ)".dimmed().to_string()) }
                        else if base_id == local_id { (BranchDisplayStatus::Behind, "(要プル)".dimmed().to_string()) }
                        else { (BranchDisplayStatus::Diverged, "(分岐)".dimmed().to_string()) }
                    }
                    Err(_) => (BranchDisplayStatus::LocalOnly, String::new()),
                }
            }
        }
        _ => (BranchDisplayStatus::LocalOnly, String::new()),
    }
}

// ブランチの表示状態を取得するヘルパー関数
pub fn get_branch_display_info(branch_name: &str, is_current: bool, uncommitted_changes: bool, remote_url: &str, git_command: &dyn GitCommandTrait) -> (String, String) {
    let mut local_id = String::new();
    handle_command_result(git_command.rev_parse_commit_id(branch_name), |id| local_id = id);

    let (status, note) = if !remote_url.is_empty() && !local_id.is_empty() {
        get_branch_display_status(branch_name, &local_id, git_command)
    } else {
        (BranchDisplayStatus::LocalOnly, String::new())
    };
    
    let display_str = if is_current {
        format!("* {} {}", branch_name.cyan().bold(), if uncommitted_changes { "*".yellow().bold() } else { "".normal() })
    } else {
        match status {
            BranchDisplayStatus::Synced => format!("  {}", branch_name.blue()),
            BranchDisplayStatus::LocalOnly | BranchDisplayStatus::Ahead | BranchDisplayStatus::Behind | BranchDisplayStatus::Diverged => {
                format!("  {}", branch_name.truecolor(255,165,0)) // オレンジ
            }
        }
    };
    (display_str, note)
}

// コンフリクト時の新しいブランチ作成を提案する関数
pub fn handle_conflict_and_offer_new_branch(operation_name: &str, git_command: &dyn GitCommandTrait) {
    eprintln!("警告: {} に失敗しました。コンフリクトの可能性があります。", operation_name.yellow());
    // confirm が削除されたため、直接 prompt_input を使用
    if prompt_input(&format!("この状態で新しいブランチを作成して変更を保持しますか？ (y/N): ")).eq_ignore_ascii_case("y") {
        let new_branch_name = prompt_input("新しいブランチ名: ");
        if new_branch_name.is_empty() {
            eprintln!("{}", "エラー: ブランチ名が入力されませんでした。".red());
            // ここで exit(1) するか、この関数が Result を返すように変更することも考えられるが、
            // 今回は元のロジック（エラー表示のみで続行、ただしその後の処理で問題が生じる可能性）を維持
        } else {
            let mut already_exists = false;
            handle_command_result(git_command.rev_parse_verify(&new_branch_name), |exists| already_exists = exists);
            if already_exists {
                eprintln!("エラー: ブランチ '{}' は既に存在します。", new_branch_name.bold().red());
                exit(1);
            }
            // handle_command_result_void が削除されたため、直接 handle_command_result を使用
            handle_command_result(git_command.checkout_b(&new_branch_name), |_| {
                println!("新しいブランチ '{}' を作成し切り替えました。", new_branch_name.cyan());
                println!("コンフリクトを解決し、再度 {} を試みてください。", operation_name.yellow());
                exit(0); // 成功時はここで終了
            });
        }
    } else {
        println!("新しいブランチは作成しませんでした。手動で状況を確認してください。");
        exit(1); // 操作キャンセル時も終了
    }
}

// --- 以下は以前のリファクタリングで「utils.rs に追加されると想定される関数群」として提示されたもの ---
// これらは utils.rs の一部として既に適切と判断し、大きな変更は加えない

pub fn prompt_non_empty_input(message: &str, error_if_empty: &str) -> String {
    let input = prompt_input(message);
    if input.is_empty() {
        eprintln!("{}", error_if_empty.red());
        std::process::exit(1);
    }
    input
}

pub struct SelectOption<V: Clone> { pub display: String, pub value: V }

pub fn prompt_select<V: ToString + Clone + Default + 'static>(message: &str, options: Vec<SelectOption<V>>) -> V {
    let mut term = Term::default();
    let mut theme = MinimalTheme::default();
    let mut p = Promptuity::new(&mut term, &mut theme);
    let prompt_options = options.into_iter()
        .map(|opt| PromptuitySelectOption::new(opt.display, opt.value))
        .collect();
    let mut select_prompt = Select::new(message, prompt_options);
    p.begin().unwrap_or_else(|e| { eprintln!("エラー: プロンプト初期化 ({:?})", e); std::process::exit(1); });
    let value = p.prompt(&mut select_prompt).unwrap_or_else(|e| { eprintln!("エラー: 選択 ({:?})", e); std::process::exit(1); });
    if let Err(e) = p.finish() { eprintln!("警告: プロンプト終了処理 ({:?})", e); }
    value
}

pub fn ensure_branch_exists(branch_name: &str, git_command: &dyn GitCommandTrait, action_verb: &str) {
    let mut exists = false;
    handle_command_result(git_command.rev_parse_verify(branch_name), |e| exists = e);
    if !exists {
        eprintln!("エラー: {}ブランチ '{}' が見つかりません。", action_verb, branch_name.red());
        std::process::exit(1);
    }
}

pub fn get_origin_url(git_command: &dyn GitCommandTrait) -> String {
    git_command.remote_get_url("origin").unwrap_or_default()
}

pub fn prompt_and_push_optional(
     branch_name: &str,
     operation_verb: &str, 
     git_command: &dyn GitCommandTrait,
     needs_checkout: bool,
 ) {
     let remote_url = get_origin_url(git_command);
     if !remote_url.is_empty() {
        let prompt_msg = format!("{}したブランチ '{}' をリモート 'origin' にプッシュし追跡設定しますか？ (y/N): ", operation_verb, branch_name);
        // confirm が削除されたため、直接 prompt_input を使用
        if prompt_input(&prompt_msg).eq_ignore_ascii_case("y") {
            if needs_checkout {
                handle_command_result(git_command.checkout(branch_name), |_| {});
            }
            // handle_command_result_void が削除されたため、直接 handle_command_result を使用
            handle_command_result(git_command.push_u("origin", branch_name), |_| {});
            println!("ブランチ '{}' を 'origin/{}' へプッシュし追跡設定しました。", branch_name.cyan(), branch_name.blue());
        }
    }
}

pub fn get_branch_select_options(git_command: &dyn GitCommandTrait) -> Vec<SelectOption<String>> {
    let mut options: Vec<SelectOption<String>> = Vec::new();
    let mut seen_branches = std::collections::HashSet::new();

    // ローカルブランチを追加
    let mut local_branch_str = String::new();
    handle_command_result(git_command.branch_list_local_str(), |s| local_branch_str = s);
    for line in local_branch_str.lines() {
        let branch_name = line.trim_start_matches("* ").trim().to_string();
        if !branch_name.is_empty() && seen_branches.insert(branch_name.clone()) {
            options.push(SelectOption {
                display: format!("{} (local)", branch_name),
                value: branch_name.clone(),
            });
        }
    }

    // リモートブランチを追加
    let mut all_branch_str = String::new();
    handle_command_result(git_command.branch_list_all_str(), |s| all_branch_str = s);
    for line in all_branch_str.lines() {
        let trimmed_line = line.trim_start_matches("* ").trim();
        if let Some(remote_branch_name_part) = trimmed_line.strip_prefix("remotes/origin/") {
            if !remote_branch_name_part.contains("HEAD ->") {
                let full_remote_name = format!("origin/{}", remote_branch_name_part);
                if seen_branches.insert(full_remote_name.clone()) {
                    options.push(SelectOption {
                        display: full_remote_name.clone(),
                        value: full_remote_name.clone(),
                    });
                }
            }
        }
    }
    options.sort_by(|a, b| a.display.cmp(&b.display));
    options
}
