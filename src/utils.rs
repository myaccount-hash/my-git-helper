use anyhow::{bail, Result, Context};
use colored::*;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

// --- 型定義 ---
#[derive(Debug, Clone)]
pub struct SelectOption<V: Clone> {
    pub display: String,
    pub value: V,
}

impl<V: Clone> ToString for SelectOption<V> {
    fn to_string(&self) -> String {
        // display文字列に色を付けることも可能だが、FuzzySelectのテーマとの相性も考慮
        self.display.clone()
    }
}

#[derive(PartialEq, Debug)]
pub enum BranchDisplayStatus { Synced, LocalOnly, Ahead, Behind, Diverged }

pub struct ResolvedBranchInfo {
    pub local_candidate_name: String,
    pub local_exists: bool,
    pub remote_tracking_candidate_name: String,
    pub remote_tracking_exists: bool,
}

// --- GitCommandTrait (メソッドはanyhow::Resultを返すように変更) ---
pub trait GitCommandTrait {
    fn symbolic_ref_head(&self) -> Result<String>;
    fn rev_parse_verify(&self, ref_name: &str) -> Result<bool>;
    fn rev_parse_commit_id(&self, ref_name: &str) -> Result<String>;
    fn merge_base(&self, commit1: &str, commit2: &str) -> Result<String>;
    fn checkout_b(&self, branch: &str) -> Result<()>;
    fn remote_get_url(&self, remote: &str) -> Result<String>;
    fn checkout(&self, branch: &str) -> Result<()>;
    fn push_u(&self, remote: &str, branch: &str) -> Result<()>;
    // 必要に応じて cmds.rs で crate::GitCommand::method() を直接呼ぶ代わりに Trait に追加
    // 例: fn branch_list_all_str(&self) -> Result<String>;
    // 例: fn branch_list_local_str(&self) -> Result<String>;
    // 例: fn status_porcelain_v1(&self) -> Result<String>;
}

// --- プロンプト系ヘルパー (戻り値をanyhow::Resultに変更) ---
// promptuity は dialoguer に置き換えられるため、関連するuseは削除
// use promptuity::prompts::{Input, Select, SelectOption as PromptuitySelectOption};
// use promptuity::themes::MinimalTheme;
// use promptuity::{Promptuity, Term};

pub fn prompt_input(message: &str) -> Result<String> {
    // dialoguer::Input を使用する例
    let input = dialoguer::Input::<String>::new()
        .with_prompt(message)
        .interact_text()?; // interact_text は Result<String, Error> を返す
    Ok(input)
}

pub fn prompt_non_empty_input(message: &str, error_if_empty: &str) -> Result<String> {
    let input = prompt_input(message)?;
    if input.is_empty() {
        bail!("{}", error_if_empty.red());
    }
    Ok(input)
}

pub fn prompt_confirm(message: &str) -> Result<bool> {
    let val = dialoguer::Confirm::new()
        .with_prompt(message)
        .default(false) // デフォルトは No
        .interact()?;
    Ok(val)
}

pub fn prompt_fuzzy_select<V: Clone>(prompt: &str, options: &[SelectOption<V>]) -> Result<Option<V>> {
    let theme = ColorfulTheme::default();
    let selection = FuzzySelect::with_theme(&theme)
        .with_prompt(prompt)
        .items(&options.iter().map(|o| &o.display).collect::<Vec<_>>())
        .default(0)
        .interact_opt()?;

    Ok(selection.map(|i| options[i].value.clone()))
}

// --- ブランチ選択肢生成ヘルパー ---
pub fn get_branch_select_options_for_fuzzy() -> Result<Vec<SelectOption<String>>> {
    let mut options: Vec<SelectOption<String>> = Vec::new();
    let mut processed_values = std::collections::HashSet::new();

    // ローカルブランチ
    let local_branch_str = crate::GitCommand::branch_list_local_str().context("ローカルブランチリストの取得に失敗")?;
    for line in local_branch_str.lines() {
        let branch_name = line.trim_start_matches("* ").trim().to_string();
        if !branch_name.is_empty() && !branch_name.contains("->") {
            if processed_values.insert(branch_name.clone()) {
                options.push(SelectOption {
                    display: format!("{} (local)", branch_name),
                    value: branch_name.clone(),
                });
            }
        }
    }

    // リモートブランチ (origin)
    // crate::GitCommand::remote_get_url("origin").is_ok() などで origin の存在を確認しても良い
    let all_branch_str = crate::GitCommand::branch_list_all_str().context("全ブランチリストの取得に失敗")?;
    for line in all_branch_str.lines() {
        let trimmed_line = line.trim_start_matches("* ").trim();
        if let Some(remote_name_part) = trimmed_line.strip_prefix("remotes/origin/") {
            if !remote_name_part.contains("HEAD ->") { // "HEAD -> origin/main" のような行は除外
                let remote_branch_name_only = remote_name_part.to_string();
                let value_for_remote = format!("origin/{}", remote_branch_name_only);

                // processed_values にはローカルブランチ名 (`my-feature`) と
                // リモートブランチの完全名 (`origin/my-feature`) の両方が入る可能性がある。
                // ここでは、valueとして `origin/` プレフィックス付きを使い、表示を工夫する。
                if processed_values.insert(value_for_remote.clone()) {
                    options.push(SelectOption {
                        display: format!("{} (origin)", remote_branch_name_only), // 表示は 'my-feature (origin)'
                        value: value_for_remote, // 値は 'origin/my-feature'
                    });
                } else {
                    // もしローカルにも同名ブランチがある場合 (value が 'my-feature' で登録済みのケース)
                    // ここで options 内の該当エントリを探して display を更新することもできるが、複雑になる。
                    // 例えば、 "my-feature (local, origin)" のように。
                    // 簡単なのは、ローカルとリモートを別のエントリとして表示すること。
                    // processed_values は value の一意性を担保するため、
                    // ローカルの 'my-feature' とリモートの 'origin/my-feature' は別物として扱われる。
                }
            }
        }
    }
    options.sort_by(|a, b| a.display.cmp(&b.display));
    Ok(options)
}


// --- その他Git関連ヘルパー (戻り値をanyhow::Resultに変更) ---
pub fn resolve_branch_input(name_from_user: &str, git_command: &dyn GitCommandTrait) -> Result<ResolvedBranchInfo> {
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
        local_exists = git_command.rev_parse_verify(&local_candidate_name)
            .context(format!("ローカルブランチ '{}' の状態確認に失敗", local_candidate_name))?;
    }
    
    let remote_tracking_exists = git_command.rev_parse_verify(&remote_tracking_candidate_name)
        .context(format!("リモート追跡ブランチ '{}' の状態確認に失敗", remote_tracking_candidate_name))?;

    Ok(ResolvedBranchInfo {
        local_candidate_name,
        local_exists,
        remote_tracking_candidate_name,
        remote_tracking_exists,
    })
}

pub fn get_current_branch_name(git_command: &dyn GitCommandTrait) -> Result<String> {
    git_command.symbolic_ref_head().context("現在のブランチ名の取得に失敗")
}

pub fn get_branch_display_status(local_branch: &str, local_id: &str, git_command: &dyn GitCommandTrait) -> Result<(BranchDisplayStatus, String)> {
    let remote_tracking_branch = format!("origin/{}", local_branch);

    let remote_exists = git_command.rev_parse_verify(&remote_tracking_branch)
        .context(format!("リモート追跡ブランチ '{}' の存在確認に失敗", remote_tracking_branch))?;

    if !remote_exists {
        return Ok((BranchDisplayStatus::LocalOnly, String::new()));
    }

    let remote_id = git_command.rev_parse_commit_id(&remote_tracking_branch)
        .context(format!("リモート追跡ブランチ '{}' のコミットID取得に失敗", remote_tracking_branch))?;
        
    if remote_id.is_empty() { // rev_parse_commit_id が空を返すことは通常ないはずだが念のため
        return Ok((BranchDisplayStatus::LocalOnly, String::new()));
    }

    if local_id == remote_id {
        Ok((BranchDisplayStatus::Synced, String::new()))
    } else {
        let base_id = git_command.merge_base(local_id, &remote_id)
            .context(format!("'{}' と '{}' の共通祖先の検索に失敗", local_branch, remote_tracking_branch))?;
        
        if base_id == remote_id { Ok((BranchDisplayStatus::Ahead, "(要プッシュ)".dimmed().to_string())) }
        else if base_id == local_id { Ok((BranchDisplayStatus::Behind, "(要プル)".dimmed().to_string())) }
        else { Ok((BranchDisplayStatus::Diverged, "(分岐)".dimmed().to_string())) }
    }
}

pub fn get_branch_display_info(branch_name: &str, is_current: bool, uncommitted_changes: bool, remote_url: &str, git_command: &dyn GitCommandTrait) -> Result<(String, String)> {
    let local_id = git_command.rev_parse_commit_id(branch_name)
        .context(format!("ブランチ '{}' のコミットID取得に失敗", branch_name))?;

    let (status, note) = if !remote_url.is_empty() && !local_id.is_empty() {
        get_branch_display_status(branch_name, &local_id, git_command)?
    } else {
        (BranchDisplayStatus::LocalOnly, String::new())
    };
    
    let display_str = if is_current {
        format!("* {} {}", branch_name.cyan().bold(), if uncommitted_changes { "*".yellow().bold() } else { "".normal() })
    } else {
        match status {
            BranchDisplayStatus::Synced => format!("  {}", branch_name.blue()),
            _ => format!("  {}", branch_name.truecolor(255,165,0)), // オレンジ (LocalOnly, Ahead, Behind, Diverged)
        }
    };
    Ok((display_str, note))
}

pub fn ensure_branch_exists(branch_name: &str, git_command: &dyn GitCommandTrait, action_verb: &str) -> Result<()> {
    if !git_command.rev_parse_verify(branch_name)
        .context(format!("{}ブランチ '{}' の状態確認に失敗", action_verb, branch_name))? {
        bail!("エラー: {}ブランチ '{}' が見つかりません。", action_verb, branch_name.red());
    }
    Ok(())
}

pub fn ensure_branch_not_exists(branch_name: &str, git_command: &dyn GitCommandTrait, entity_description: &str) -> Result<()> {
    if git_command.rev_parse_verify(branch_name)
        .context(format!("{} '{}' の状態確認に失敗", entity_description, branch_name))? {
        bail!("エラー: {} '{}' は既に存在します。", entity_description, branch_name.red());
    }
    Ok(())
}

pub fn get_origin_url(git_command: &dyn GitCommandTrait) -> Result<String> {
    // remote_get_url がエラーを返す可能性があるので、それをそのまま伝播させるか、
    // もし 'origin' がない場合に空文字列を返したいなら .unwrap_or_default() 的な処理が必要。
    // ここではエラーを伝播させる。なければエラーになる。
    // もし origin がなくても処理を続けたい場合は呼び出し側で .ok() や match で処理。
    // シンプルにするため、ここではエラーはエラーとして扱う。
    // もし存在しない場合に空を返したいなら、以下のようにする:
    // match git_command.remote_get_url("origin") {
    //     Ok(url) => Ok(url),
    //     Err(_) => Ok(String::new()), // エラーの種類によって判断が必要な場合もある
    // }
    // ここでは、main.rs の GitCommand::remote_get_url がエラーを返すので、それをそのまま使う。
    git_command.remote_get_url("origin")
}

pub fn prompt_and_push_optional(
     branch_name: &str,
     operation_verb: &str,
     git_command: &dyn GitCommandTrait,
     needs_checkout: bool,
 ) -> Result<()> {
    match get_origin_url(git_command) {
        Ok(remote_url) if !remote_url.is_empty() => {
            let prompt_msg = format!("{}したブランチ '{}' をリモート 'origin' にプッシュし追跡設定しますか？", operation_verb, branch_name);
            if prompt_confirm(&prompt_msg)? { // prompt_confirm は (y/N) を表示しないので調整が必要かも
                if needs_checkout {
                    git_command.checkout(branch_name).context(format!("ブランチ '{}' への切り替えに失敗", branch_name))?;
                }
                git_command.push_u("origin", branch_name).context(format!("ブランチ '{}' のプッシュに失敗", branch_name))?;
                println!("ブランチ '{}' を 'origin/{}' へプッシュし追跡設定しました。", branch_name.cyan(), branch_name.blue());
            }
        }
        Ok(_) => { /* リモートURLが空 (設定なし) */ println!("{}", "リモート 'origin' が未設定のため、プッシュはスキップしました。".yellow()); },
        Err(_) => { /* get_origin_url でエラー (例えばgitコマンド失敗) */ println!("{}", "リモート情報の取得に失敗したため、プッシュはスキップしました。".yellow()); }
    }
    Ok(())
}

pub fn handle_conflict_and_offer_new_branch(operation_name: &str, git_command: &dyn GitCommandTrait) -> Result<()> {
    eprintln!("警告: {} に失敗しました。コンフリクトの可能性があります。", operation_name.yellow());
    let confirm_msg = "この状態で新しいブランチを作成して変更を保持しますか？";
    if prompt_confirm(&confirm_msg)? {
        let new_branch_name = prompt_non_empty_input("新しいブランチ名: ", "エラー: ブランチ名が入力されませんでした。")?;
        
        ensure_branch_not_exists(&new_branch_name, git_command, "ブランチ")?;
            
        git_command.checkout_b(&new_branch_name)
            .context(format!("新しいブランチ '{}' の作成と切り替えに失敗", new_branch_name))?;

        println!("新しいブランチ '{}' を作成し切り替えました。", new_branch_name.cyan());
        println!("コンフリクトを解決し、再度 {} を試みてください。", operation_name.yellow());
    } else {
        // 新しいブランチを作成しない場合、エラーとして扱う
        bail!("新しいブランチは作成しませんでした。手動で状況を確認してください。");
    }
    Ok(())
}