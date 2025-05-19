#!/bin/bash
# ga - 個人的なGithub操作ヘルパー
# fzfを利用したGithubの操作を支援するシェルスクリプト

# --- ヘルプ表示 ---
_ga_show_help() {
  echo "使用方法: ga [コマンド]"
  echo "コマンド一覧:"
  echo "  repo          GitHubリポジトリ操作"
  echo "  branch        ブランチ操作"
  echo "  tag           タグ操作"
  echo "  stage         ステージング操作 (add/unstage/status/diff)"
  echo "  commit        コミット操作 (ステージング確認あり)"
  echo "  stash         スタッシュ操作"
  echo "  remote        ローカルリポジトリのリモート接続先管理"
  echo "  pr            プルリクエスト操作"
  echo "  issue         Issue操作"
  echo "  diff          差分表示"
  echo "  init          カレントディレクトリをGitリポジトリとして初期化"
  echo "  help          このヘルプを表示"
  echo ""
  echo "fzfの一覧表示中: Enterで操作選択。一部操作ではTabキーで複数選択も可能です。"
  echo "新規作成は各一覧の先頭の '[+] 新規...' を選択してください。"
}

# --- 補助関数: インタラクティブにファイルを追加 ---
_ga_interactive_add_files() {
    echo "ステージングするファイル/ディレクトリを選択してください (Tabで複数選択):"
    local files_to_add
    files_to_add=$(command git status -s | grep -E "^\s*[MADRCU]|^\?\?" | awk '{$1=""; print $0}' | sed 's/^[ \t]*//' | fzf -m --height=40% --header "ステージングするアイテムを選択 (Tab:複数, Enter:決定)")
    if [ -n "$files_to_add" ]; then
        echo "$files_to_add" | xargs -d '\n' --no-run-if-empty command git add --
        echo "--- 選択されたアイテムをステージングしました ---"
        command git status -s
    else
        echo "ステージングするファイルは選択されませんでした。"
    fi
}

# --- 補助関数: ステージングされていない変更の確認と対応 (コミット/プッシュ前) ---
_ga_check_uncommitted_changes_before_action() {
  local action_description=${1:-操作} 
  local auto_proceed_if_clean=${2:-false} 

  local work_tree_changes=false
  local staged_changes=false

  if ! command git diff --quiet; then work_tree_changes=true; fi
  if ! command git diff --cached --quiet; then staged_changes=true; fi

  if ! $work_tree_changes && ! $staged_changes; then
    if ! $auto_proceed_if_clean && [[ "$action_description" == "コミット" ]]; then
        echo "${action_description}する変更がステージングされていません。"
        local choice_no_changes
        choice_no_changes=$(printf "今すぐステージングする\nキャンセル" | fzf --height=20% --header "コミットする変更がありません。")
        if [[ "$choice_no_changes" == "今すぐステージングする" ]]; then
            _ga_interactive_add_files
            echo "ステージングしました。再度${action_description}操作を実行してください。"
        else
            echo "${action_description}をキャンセルしました。"
        fi
        return 1 
    fi
    return 0 
  fi

  echo "====== 未コミットの変更があります ======"
  command git status -s
  echo "======================================"

  local options=()
  if $work_tree_changes; then options+=("変更をステージングする"); fi
  if $staged_changes || $work_tree_changes ; then 
    options+=("このまま${action_description}する (ステージ済みの変更のみ)")
  fi
  options+=("操作をキャンセルする")

  local choice
  choice=$(printf "%s\n" "${options[@]}" | fzf --height=30% --header "未コミットの変更があります。どうしますか？")

  case "$choice" in
    "変更をステージングする")
      _ga_interactive_add_files
      echo "ステージングしました。${action_description}を続行するには再度操作を行ってください。"
      return 1 
      ;;
    "このまま${action_description}する (ステージ済みの変更のみ)")
      if ! $staged_changes && ! command git diff --cached --quiet ; then 
          echo "ステージングされている変更がありません。${action_description}を中止します。"
          return 1
      fi
      echo "現在のステージング内容で${action_description}を続行します。"
      return 0 
      ;;
    "操作をキャンセルする" | *)
      echo "${action_description}をキャンセルしました。"
      return 1 
      ;;
  esac
}

# --- ステージング操作 ---
_ga_handle_stage() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi

  local ops=("ファイルを追加 (add)" "ステージング解除 (unstage)" "ステータス表示 (status)" "ステージ済み差分 (diff --staged)")
  local choice
  choice=$(printf "%s\n" "${ops[@]}" | fzf --height=30% --header "ステージング操作を選択")

  if [ -z "$choice" ]; then echo "キャンセルしました。"; return; fi

  case "$choice" in
    "ファイルを追加 (add)")
      _ga_interactive_add_files
      ;;
    "ステージング解除 (unstage)")
      echo "ステージング解除するファイルを選択してください (Tabで複数選択):"
      local files_to_unstage
      files_to_unstage=$(command git diff --name-only --cached | fzf -m --height=40% --header "ステージング解除するファイルを選択 (Tab:複数, Enter:決定)")
      if [ -n "$files_to_unstage" ]; then
        echo "$files_to_unstage" | xargs -d '\n' --no-run-if-empty command git reset HEAD --
        echo "--- 選択されたファイルのステージングを解除しました ---"
        command git status -s
      else
        echo "ステージング解除はキャンセルされました。"
      fi
      ;;
    "ステータス表示 (status)")
      command git status
      ;;
    "ステージ済み差分 (diff --staged)")
      command git diff --staged --color=always | less -R
      ;;
  esac
}

# --- リポジトリ操作 ---
_ga_repo_create() {
  echo -n "新規リポジトリ名: "; read -r name
  if [ -z "$name" ]; then echo "名前は必須です。"; return 1; fi
  echo -n "説明 (任意): "; read -r desc
  echo -n "可視性 (public/private, デフォルト: private): "; read -r vis
  vis=${vis:-private}
  command gh repo create "$name" --description "$desc" "--$vis" && echo "リポジトリ '$name' を作成しました。" || echo "作成に失敗しました。"
}

_ga_handle_repo() {
  echo "リポジトリ情報取得中..."
  local repo_data_raw; local exit_status

  repo_data_raw=$(command gh repo list --limit 200 --json 'nameWithOwner,isArchived,visibility,description' \
    --jq '.[] | "\(.nameWithOwner)\t\(.isArchived)\t\(.visibility // "-")\t\(.description // "-")"' 2>&1)
  exit_status=$?

  if [ $exit_status -ne 0 ]; then
    echo "エラー: 'gh repo list' コマンドの実行に失敗しました (終了ステータス: $exit_status)。" >&2
    echo "ghコマンドからの出力:" >&2
    echo "$repo_data_raw" >&2
    return 1
  fi
  
  local create_new_option="[+] 新規リポジトリ作成"
  local fzf_input="$create_new_option"
  if [ -n "$repo_data_raw" ]; then fzf_input+=$'\n'"$repo_data_raw"; 
  elif [ $exit_status -eq 0 ]; then echo "リポジトリが見つかりませんでした。(新規作成は可能です)"; fi

  local selected_lines_str 
  selected_lines_str=$(echo -e "$fzf_input" | \
    fzf --height=50% --header "リポジトリ (Tab:複数選択, Enter:操作選択)" --multi \
        --preview "if [ \"{}\" == \"$create_new_option\" ]; then echo '新しいリポジトリを作成します。'; else echo {} | awk '{print \$1}' | xargs -I {} command gh repo view {}; fi")

  if [ -z "$selected_lines_str" ]; then echo "キャンセルしました (fzfが空を返しました)。"; return; fi

  local processed_fzf_output
  processed_fzf_output=$(echo -e "$selected_lines_str" | sed '/^$/d') # 空行を除去

  if [ -z "$processed_fzf_output" ]; then
      echo "有効な選択がありませんでした (空行のみ選択か、予期せぬエラー)。"
      return
  fi

  # mapfile の代替: while read ループで配列に格納
  local selected_lines=()
  while IFS= read -r line; do
    selected_lines+=("$line")
  done < <(echo -e "$processed_fzf_output")
  
  local create_selected_in_array=false
  for item in "${selected_lines[@]}"; do
    if [[ "$item" == "$create_new_option" ]]; then
      create_selected_in_array=true
      break
    fi
  done

  if $create_selected_in_array; then
      if [ ${#selected_lines[@]} -gt 1 ]; then
          echo "新規作成は単独で選択してください。他のリポジトリとの同時選択はできません。"
      else
          _ga_repo_create
      fi
      echo "ヒント: リストを更新するには、再度 'ga repo' を実行してください。"
      return
  fi
  
  local num_selected=${#selected_lines[@]}
  if [ $num_selected -eq 0 ]; then 
      echo "何も有効なリポジトリが選択されませんでした (配列化後0件)。"
      return
  fi

  if [ $num_selected -eq 1 ]; then
    local selected_line="${selected_lines[0]}"
    local repo_name=$(echo "$selected_line" | awk '{print $1}')
    local is_archived_str=$(echo "$selected_line" | awk '{print $2}')
    local is_archived=false; [[ "$is_archived_str" == "true" ]] && is_archived=true
    local archive_text=$($is_archived && echo "アーカイブ解除" || echo "アーカイブ")
    
    local ops=("クローン" "$archive_text" "削除" "ブラウザで表示" "詳細表示(CLI)")
    local action=$(printf "%s\n" "${ops[@]}" | fzf --height=40% --header "'$repo_name' 操作選択")
    if [ -z "$action" ]; then echo "キャンセルしました。"; return; fi

    case "$action" in
      "クローン")
        local dir; echo -n "クローン先 (デフォルト:リポジトリ名): "; read -r dir
        command gh repo clone "$repo_name" "${dir:-$(basename "$repo_name")}" && echo "クローン完了。"
        ;;
      "$archive_text")
        if $is_archived; then
          command gh api -X PATCH "repos/$repo_name" -f archived=false --silent && echo "'$repo_name': アーカイブ解除しました。"
        else
          command gh repo archive "$repo_name" --yes && echo "'$repo_name': アーカイブしました。"
        fi
        ;;
      "削除")
        echo -n "'$repo_name' を削除しますか？ (yesで実行): "; read -r confirm
        if [[ "$confirm" == "yes" ]]; then
          command gh repo delete "$repo_name" --yes && echo "'$repo_name': 削除しました。"
        else echo "中止しました。"; fi
        ;;
      "ブラウザで表示") command gh repo view "$repo_name" --web ;;
      "詳細表示(CLI)") command gh repo view "$repo_name" ;;
    esac
  elif [ $num_selected -gt 1 ]; then
    echo "$num_selected 件のリポジトリが選択されました。"
    for line in "${selected_lines[@]}"; do echo "  - $(echo "$line" | awk '{print $1}')"; done

    local bulk_ops=("一括アーカイブ (未アーカイブのもの)" "一括アーカイブ解除 (アーカイブ済みのもの)" "一括削除 (各個確認なし)" "キャンセル")
    local bulk_action=$(printf "%s\n" "${bulk_ops[@]}" | fzf --height=30% --header "選択した $num_selected 件への一括操作")

    if [ -z "$bulk_action" ] || [ "$bulk_action" == "キャンセル" ]; then echo "一括操作をキャンセルしました。"; return; fi

    if [[ "$bulk_action" == "一括削除 (各個確認なし)" ]]; then
        echo -n "本当に選択された $num_selected 件のリポジトリ全てを削除しますか？この操作は元に戻せません！ (YES と大文字で入力): "
        read -r confirm_all_delete
        if [[ "$confirm_all_delete" != "YES" ]]; then echo "一括削除を中止しました。"; return; fi
    fi

    echo "一括操作を開始します..."
    for selected_line_bulk in "${selected_lines[@]}"; do
      local repo_name_bulk=$(echo "$selected_line_bulk" | awk '{print $1}')
      local is_archived_str_bulk=$(echo "$selected_line_bulk" | awk '{print $2}')
      local is_archived_bulk=false; [[ "$is_archived_str_bulk" == "true" ]] && is_archived_bulk=true

      echo "--- 処理中: $repo_name_bulk ---"
      case "$bulk_action" in
        "一括アーカイブ (未アーカイブのもの)")
          if $is_archived_bulk; then echo "既にアーカイブ済み。スキップ。"
          else command gh repo archive "$repo_name_bulk" --yes && echo "アーカイブ成功。" || echo "アーカイブ失敗。"; fi ;;
        "一括アーカイブ解除 (アーカイブ済みのもの)")
          if ! $is_archived_bulk; then echo "アーカイブされていません。スキップ。"
          else command gh api -X PATCH "repos/$repo_name_bulk" -f archived=false --silent && echo "アーカイブ解除成功。" || echo "アーカイブ解除失敗。"; fi ;;
        "一括削除 (各個確認なし)")
          command gh repo delete "$repo_name_bulk" --yes && echo "削除成功。" || echo "削除失敗。" ;;
      esac
    done
    echo "-----------------------------"
    echo "一括操作が完了しました。"
  fi
}

# --- ブランチ操作 ---
_ga_branch_create() {
  echo -n "新規ブランチ名: "; read -r name
  if [ -z "$name" ]; then echo "名前は必須です。"; return 1; fi
  echo -n "作成元 (任意, デフォルト:カレント): "; read -r base
  if [ -n "$base" ]; then command git checkout -b "$name" "$base"; else command git checkout -b "$name"; fi
  echo "ブランチ '$name' を作成し切り替えました。"
}

_ga_handle_branch() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi
  
  local create_new_option="[+] 新規ブランチ作成"
  local list_cmd="command git branch -a --sort=-committerdate --format='%(if:equals=HEAD)%(refname:short)%(then)* %(else)  %(end)%(refname:short)' | sed 's|remotes/origin/||' | awk '!seen[\$0]++'"

  local selected_line
  selected_line=$( (echo "$create_new_option"; eval "$list_cmd") | \
    fzf --height=50% --header "ブランチ (Enter:操作選択)" \
        --preview "if [ \"{}\" == \"$create_new_option\" ]; then echo '新しいブランチを作成します。'; else command git log --oneline --graph --decorate --color=always {2}; fi")

  if [ -z "$selected_line" ]; then echo "キャンセルしました。"; return; fi
  
  if [ "$selected_line" == "$create_new_option" ]; then
    _ga_branch_create
    echo "ヒント: リストを更新するには、再度 'ga branch' を実行してください。"
    return
  fi
  
  local branch=$(echo "$selected_line" | sed 's/^\* //; s/^  //') 
  local current=$(command git branch --show-current)

  local ops=("切り替え")
  [[ "$branch" != "$current" ]] && ops+=("「$current」へマージ")
  ops+=("プッシュ(origin)")
  [[ "$branch" != "$current" ]] && ops+=("ローカル削除")
  ops+=("リモート削除(origin)")
  ops+=("名前変更")
  
  local action=$(printf "%s\n" "${ops[@]}" | fzf --height=40% --header "'$branch' 操作選択")
  if [ -z "$action" ]; then echo "キャンセルしました。"; return; fi

  case "$action" in
    "切り替え") command git checkout "$branch" ;;
    "「$current」へマージ") command git merge "$branch" ;;
    "プッシュ(origin)")
        if ! _ga_check_uncommitted_changes_before_action "プッシュ" true; then return; fi
        command git push origin "$branch"
        ;;
    "ローカル削除")
      if [[ "$branch" == "$current" ]]; then echo "カレントブランチは削除できません。"; return; fi
      echo -n "'$branch' を削除しますか？(完全に削除:D, マージ済み:d): "; read -r opt
      if [[ "$opt" == "D" || "$opt" == "d" ]]; then command git branch "-$opt" "$branch"; fi
      ;;
    "リモート削除(origin)")
      echo -n "'origin/$branch' を削除しますか？ (yesで実行): "; read -r confirm
      if [[ "$confirm" == "yes" ]]; then command git push origin --delete "$branch"; fi
      ;;
    "名前変更")
      echo -n "新しいブランチ名: "; read -r new_name
      if [ -n "$new_name" ]; then command git branch -m "$branch" "$new_name"; fi
      ;;
  esac
}

# --- タグ操作 ---
_ga_tag_create() {
    echo -n "新規タグ名 (例: v1.0.0): "; read -r name
    if [ -z "$name" ]; then echo "タグ名は必須です。"; return 1; fi
    echo -n "アノテーションメッセージ (空なら軽量タグ): "; read -r msg
    if [ -n "$msg" ]; then command git tag -a "$name" -m "$msg"; else command git tag "$name"; fi
    echo "タグ '$name' を作成しました。"
}

_ga_handle_tag() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi
  
  local create_new_option="[+] 新規タグ作成"
  local list_cmd="command git tag --sort=-v:refname" 

  local selected
  selected=$( (echo "$create_new_option"; eval "$list_cmd") | \
    fzf --height=50% --header "タグ (Enter:操作選択)" \
        --preview "if [ \"{}\" == \"$create_new_option\" ]; then echo '新しいタグを作成します。'; else command git show --color=always {}; fi")

  if [ -z "$selected" ]; then echo "キャンセルしました。"; return; fi

  if [ "$selected" == "$create_new_option" ]; then
    _ga_tag_create
    echo "ヒント: リストを更新するには、再度 'ga tag' を実行してください。"
    return
  fi

  local ops=("詳細表示" "プッシュ(origin)" "ローカル削除" "リモート削除(origin)")
  local action=$(printf "%s\n" "${ops[@]}" | fzf --height=30% --header "'$selected' 操作選択")
  if [ -z "$action" ]; then echo "キャンセルしました。"; return; fi

  case "$action" in
    "詳細表示") command git show "$selected" | less -R ;;
    "プッシュ(origin)") command git push origin "$selected" ;;
    "ローカル削除")
      echo -n "'$selected' を削除しますか？ (yesで実行): "; read -r confirm
      if [[ "$confirm" == "yes" ]]; then command git tag -d "$selected"; fi
      ;;
    "リモート削除(origin)")
      echo -n "'origin/$selected' を削除しますか？ (yesで実行): "; read -r confirm
      if [[ "$confirm" == "yes" ]]; then command git push origin --delete "$selected"; fi
      ;;
  esac
}

# --- コミット操作 ---
_ga_handle_commit() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi
  
  if ! _ga_check_uncommitted_changes_before_action "コミット"; then return; fi

  local ops=("新規作成 (メッセージ入力)" "直前を修正 (--amend)" "履歴表示 (git log)")
  local action=$(printf "%s\n" "${ops[@]}" | fzf --height=30% --header "コミット操作を選択")
  if [ -z "$action" ]; then echo "キャンセルしました。"; return; fi

  case "$action" in
    "新規作成 (メッセージ入力)")
      echo -n "コミットメッセージ: "; read -r msg
      if [ -n "$msg" ]; then command git commit -m "$msg"; fi
      ;;
    "直前を修正 (--amend)")
      echo "ステージングエリアの変更が直前のコミットに追加/修正されます。"
      echo -n "メッセージも修正しますか？(y/N): "; read -r amend_msg
      if [[ "$amend_msg" == "y" || "$amend_msg" == "Y" ]]; then command git commit --amend; 
      else command git commit --amend --no-edit; fi
      ;;
    "履歴表示 (git log)")
      command git log --oneline --graph --decorate --color=always | \
        fzf --height=70% --header "コミット履歴 (Enter:詳細)" \
            --preview "command git show --color=always {1}" | awk '{print $1}' | xargs -I{} command git show --color=always {} | less -R
      ;;
  esac
}

# --- スタッシュ操作 ---
_ga_stash_create() {
    echo -n "スタッシュメッセージ(任意): "; read -r msg
    command git stash push -m "${msg:-Stash-$(date +%Y%m%d-%H%M%S)}"
    echo "現在の変更をスタッシュしました。"
}

_ga_handle_stash() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi
  
  local create_new_option="[+] 現在の変更をスタッシュ"
  local list_cmd="command git stash list"
  
  local selected 
  selected=$( (echo "$create_new_option"; eval "$list_cmd" 2>/dev/null) | \
    fzf --height=40% --header "スタッシュ (Enter:操作選択)" \
        --preview "if [ \"{}\" == \"$create_new_option\" ]; then echo '現在の変更を新しいスタッシュとして保存します。'; else command git stash show -p --color=always {1}; fi")
  
  if [ -z "$selected" ]; then echo "キャンセルしました。"; return; fi

  if [ "$selected" == "$create_new_option" ]; then
    _ga_stash_create
    echo "ヒント: リストを更新するには、再度 'ga stash' を実行してください。"
    return
  fi

  local stash_ref=$(echo "$selected" | awk '{print $1}' | sed 's/://') 
  local ops=("適用して削除(pop)" "適用のみ(apply)" "内容表示" "削除(drop)")
  local action=$(printf "%s\n" "${ops[@]}" | fzf --height=30% --header "'$stash_ref' 操作選択")
  if [ -z "$action" ]; then echo "キャンセルしました。"; return; fi

  case "$action" in
    "適用して削除(pop)") command git stash pop "$stash_ref" ;;
    "適用のみ(apply)") command git stash apply "$stash_ref" ;;
    "内容表示") command git stash show -p "$stash_ref" | less -R ;;
    "削除(drop)") command git stash drop "$stash_ref" ;;
  esac
}

# --- リモート設定 ---
_ga_handle_remote() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi
  echo "現在のリモート:"
  command git remote -v
  echo

  local ops=("追加" "削除" "URL変更")
  local action=$(printf "%s\n" "${ops[@]}" | fzf --height=30% --header "リモート設定 操作選択")
  if [ -z "$action" ]; then echo "キャンセルしました。"; return; fi
  
  case "$action" in
    "追加")
      echo -n "リモート名 (例: origin): "; read -r name
      echo -n "URL: "; read -r url
      if [ -n "$name" ] && [ -n "$url" ]; then command git remote add "$name" "$url"; fi
      ;;
    "削除")
      local remote_to_remove
      remote_to_remove=$(command git remote | fzf --height=30% --header "削除するリモート名を選択")
      if [ -n "$remote_to_remove" ]; then command git remote remove "$remote_to_remove"; fi
      ;;
    "URL変更")
      local remote_to_change
      remote_to_change=$(command git remote | fzf --height=30% --header "URLを変更するリモート名を選択")
      if [ -n "$remote_to_change" ]; then
        echo -n "新しいURL: "; read -r new_url
        if [ -n "$new_url" ]; then command git remote set-url "$remote_to_change" "$new_url"; fi
      fi
      ;;
  esac
  echo; echo "更新後のリモート:"
  command git remote -v
}

# --- PR操作 ---
_ga_pr_create() { command gh pr create; }
_ga_handle_pr() {
  local create_new_option="[+] 新規プルリクエスト作成"
  local list_cmd="command gh pr list --json number,title,author,updatedAt --template '{{range .}}{{.number}}\t{{.title}}\t({{.author.login}})\t{{timeFormat .updatedAt \"2006-01-02\"}}\n{{end}}'"
  
  local selected
  selected=$( (echo "$create_new_option"; eval "$list_cmd" 2>/dev/null) | \
    fzf --height=50% --header "PR (Enter:操作選択)" \
        --preview "if [ \"{}\" == \"$create_new_option\" ]; then echo '新しいプルリクエストを作成します。'; else command gh pr view {1} --comments=false; fi")

  if [ -z "$selected" ]; then echo "キャンセルしました。"; return; fi
  if [ "$selected" == "$create_new_option" ]; then _ga_pr_create; return; fi
  
  local pr_num=$(echo "$selected" | awk '{print $1}')
  local pr_ops=("詳細表示" "チェックアウト" "ブラウザで表示" "閉じる" "再オープン" "マージ")
  local pr_action=$(printf "%s\n" "${pr_ops[@]}" | fzf --height=40% --header "PR #$pr_num 操作選択")
  if [ -z "$pr_action" ]; then echo "キャンセルしました。"; return; fi
  
  case "$pr_action" in
    "詳細表示") command gh pr view "$pr_num" ;;
    "チェックアウト") command gh pr checkout "$pr_num" ;;
    "ブラウザで表示") command gh pr view "$pr_num" --web ;;
    "閉じる") command gh pr close "$pr_num" ;;
    "再オープン") command gh pr reopen "$pr_num" ;;
    "マージ") command gh pr merge "$pr_num" ;;
  esac
}

# --- Issue操作 ---
_ga_issue_create() { command gh issue create; }
_ga_handle_issue() {
  local create_new_option="[+] 新規Issue作成"
  local list_cmd="command gh issue list --json number,title,author,updatedAt --template '{{range .}}{{.number}}\t{{.title}}\t({{.author.login}})\t{{timeFormat .updatedAt \"2006-01-02\"}}\n{{end}}'"

  local selected
  selected=$( (echo "$create_new_option"; eval "$list_cmd" 2>/dev/null) | \
    fzf --height=50% --header "Issue (Enter:操作選択)" \
        --preview "if [ \"{}\" == \"$create_new_option\" ]; then echo '新しいIssueを作成します。'; else command gh issue view {1} --comments=false; fi")

  if [ -z "$selected" ]; then echo "キャンセルしました。"; return; fi
  if [ "$selected" == "$create_new_option" ]; then _ga_issue_create; return; fi
  
  local issue_num=$(echo "$selected" | awk '{print $1}')
  local issue_ops=("詳細表示" "ブラウザで表示" "閉じる" "再オープン")
  local issue_action=$(printf "%s\n" "${issue_ops[@]}" | fzf --height=30% --header "Issue #$issue_num 操作選択")
  if [ -z "$issue_action" ]; then echo "キャンセルしました。"; return; fi

  case "$issue_action" in
    "詳細表示") command gh issue view "$issue_num" ;;
    "ブラウザで表示") command gh issue view "$issue_num" --web ;;
    "閉じる") command gh issue close "$issue_num" ;;
    "再オープン") command gh issue reopen "$issue_num" ;;
  esac
}

# --- 差分表示 ---
_ga_handle_diff() {
  if ! command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then echo "Gitリポジトリ内ではありません。"; return; fi
  local ops=("ステージ済み(staged)" "作業ツリー(unstaged)" "特定ファイル" "コミット間")
  local choice=$(printf "%s\n" "${ops[@]}" | fzf --height=30% --header "差分表示の種類を選択")
  if [ -z "$choice" ]; then echo "キャンセルしました。"; return; fi

  case "$choice" in
    "ステージ済み(staged)") command git diff --staged --color=always | less -R ;;
    "作業ツリー(unstaged)") command git diff --color=always | less -R ;;
    "特定ファイル")
      local file
      file=$(command git status -s | awk '{print $2}' | fzf --height=40% --header "差分表示するファイルを選択")
      if [ -n "$file" ]; then command git diff --color=always -- "$file" | less -R; fi
      ;;
    "コミット間")
      local c1 c2 
      c1=$(command git log --oneline --color=always | fzf --height=40% --header "比較元コミット(古い方)" | awk '{print $1}')
      if [ -z "$c1" ]; then return; fi
      c2=$(command git log --oneline --color=always | fzf --height=40% --header "比較先コミット(新しい方/ブランチ名)" | awk '{print $1}')
      if [ -z "$c2" ]; then return; fi
      command git diff --color=always "$c1" "$c2" | less -R
      ;;
  esac
}

# --- Gitリポジトリ初期化 ---
_ga_handle_init() {
    if command git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
        echo "既にGitリポジトリです。"
        read -r -p "強制的に再初期化しますか？ (yes/NO): " confirm
        if [[ "$confirm" != "yes" ]]; then return; fi
    fi
    command git init && echo "カレントディレクトリにGitリポジトリを初期化しました。"
}

# --- メイン処理 ---
if [ $# -eq 0 ]; then
  options=(
    "repo:GitHubリポジトリ操作"
    "branch:ブランチ操作"
    "tag:タグ操作"
    "stage:ステージング操作"
    "commit:コミット操作"
    "stash:スタッシュ操作"
    "remote:ローカルのリモート接続先管理"
    "pr:プルリクエスト操作"
    "issue:Issue操作"
    "diff:差分表示"
    "init:Gitリポジトリ初期化"
    "help:ヘルプ表示"
    "exit:終了"
  )
  selected_line=$(printf "%s\n" "${options[@]}" | fzf --height=60% --header "実行するコマンドを選択" --delimiter=":" --with-nth=1)
  
  if [ -z "$selected_line" ]; then cmd="exit"; else cmd=$(echo "$selected_line" | cut -d: -f1); fi
else
  cmd="$1"
fi

case "$cmd" in
  repo) _ga_handle_repo ;;
  branch) _ga_handle_branch ;;
  tag) _ga_handle_tag ;;
  stage) _ga_handle_stage ;;
  commit) _ga_handle_commit ;;
  stash) _ga_handle_stash ;;
  remote) _ga_handle_remote ;;
  pr) _ga_handle_pr ;;
  issue) _ga_handle_issue ;;
  diff) _ga_handle_diff ;;
  init) _ga_handle_init ;;
  help) _ga_show_help ;;
  exit) echo "終了します。" ;;
  *) echo "不明なコマンド: $cmd"; _ga_show_help ;;
esac