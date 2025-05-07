#!/bin/bash

case "$1" in
  init)
    git init
    ;;
  remote)
    remotes=$(git remote)

    if [ -z "$2" ]; then
      # 引数なしの場合: 接続状況の確認と再接続/接続処理
      if [ -z "$remotes" ]; then
        read -p "リモートリポジトリが設定されていません。接続しますか？ (y/N): " connect_remote
        if [[ "$connect_remote" =~ ^[Yy]$ ]]; then
          read -p "リモートリポジトリのURLを入力してください: " remote_url
          if [ -n "$remote_url" ]; then
            git remote add origin "$remote_url"
            if [ $? -eq 0 ]; then
              echo "リモートリポジトリ '$remote_url' に接続しました。"
            else
              echo "エラー: リモートリポジトリへの接続に失敗しました。"
            fi
          fi
        fi
      else
        echo "設定されているリモートリポジトリのURL:"
        git remote get-url origin
        read -p "再接続しますか？ (y/N, 切断のみの場合は何も入力しないでください): " reconnect_remote
        if [[ "$reconnect_remote" =~ ^[Yy]$ ]]; then
          read -p "新しいリモートリポジトリのURLを入力してください: " new_remote_url
          if [ -n "$new_remote_url" ]; then
            git remote set-url origin "$new_remote_url"
            if [ $? -eq 0 ]; then
              echo "リモートリポジトリのURLを '$new_remote_url' に更新しました。"
            else
              echo "エラー: リモートリポジトリのURL更新に失敗しました。"
            fi
          fi
        fi
      fi
    else
      # 引数がある場合: リモートリポジトリの追加
      remote_url="$2"
      git remote add origin "$remote_url"
      if [ $? -eq 0 ]; then
        echo "リモートリポジトリ '$remote_url' を追加しました。"
      else
        echo "エラー: リモートリポジトリの追加に失敗しました。"
      fi
    fi
    ;;
  add)
    git add .
    ;;
  commit)
    read -p "コミットメッセージを入力してください: " message
    git commit -m "$message"
    ;;
  push)
    git push -u origin main
    ;;
  all)
    git add .
    read -p "コミットメッセージを入力してください: " message
    git commit -m "$message"

    current_branch=$(git symbolic-ref --short HEAD 2>/dev/null)
    tracking_branch=$(git config branch."$current_branch".remote)

    if [ -n "$tracking_branch" ] && [ "$tracking_branch" != "." ]; then
      read -p "現在のブランチ '$current_branch' は '$tracking_branch' を追跡しています。プッシュしますか？ (y/N): " push_confirm
      if [[ "$push_confirm" =~ ^[Yy]$ ]]; then
        git push
      fi
    else
      echo "現在のブランチ '$current_branch' はリモートブランチを追跡していません。プッシュはスキップします。"
    fi
    ;;
  setup)
    git init
    read -p "リモートリポジトリのURLを入力してください: " remote_url
    git remote add origin "$remote_url"
    ;;
branch)
    git fetch --prune origin # リモートの最新情報を取得
    echo "全てのブランチ"
    git branch --all | while IFS= read -r branch
    do
      if [[ "$branch" == *'origin/'* ]]; then
        echo "$branch (remote)"
      elif [[ "$branch" == \* ]]; then
        echo "${branch#* } (local)" # 先頭の '*' とスペースを削除
      else
        echo "$branch (local)"
      fi
    done
    ;;
  checkout)
    read -p "作成してチェックアウトするブランチ名を入力してください: " branch_name
    git checkout -b "$branch_name"
    ;;
  switch)
    echo "切り替え可能なブランチ:"
    git branch -a
    read -p "切り替えたいブランチ名を入力してください: " branch_name
    git checkout "$branch_name"
    ;;
  merge)
    read -p "現在のブランチにマージしたいブランチ名を入力してください: " merge_branch_name
    git merge "$merge_branch_name"
    if [ $? -eq 0 ]; then
      read -p "マージが成功しました。マージ元のブランチ '$merge_branch_name' を削除しますか？ (y/N): " delete_branch
      if [[ "$delete_branch" =~ ^[Yy]$ ]]; then
        git branch -d "$merge_branch_name"
        echo "ブランチ '$merge_branch_name' を削除しました。"
      fi
    else
      echo "マージに失敗しました。コンフリクトが発生した可能性があります。"
    fi
    ;;
save)
    echo "現在のブランチ:"
    git branch --list

    read -p "保存したいブランチ名を選択してください: " target_branch

    # ブランチが存在するか確認
    if ! git rev-parse --verify --quiet "$target_branch"; then
      echo "エラー: ブランチ '$target_branch' は存在しません。"
      exit 1
    fi

    # 必要であればチェックアウト (現在のブランチと異なる場合)
    current_branch=$(git symbolic-ref --short HEAD 2>/dev/null)
    if [ "$current_branch" != "$target_branch" ]; then
      git checkout "$target_branch"
      if [ $? -ne 0 ]; then
        echo "エラー: ブランチ '$target_branch' へのチェックアウトに失敗しました。"
        exit 1
    fi
    fi

    git add .
    read -p "コミットメッセージを入力してください: " message
    git commit -m "$message"

    # リモートにプッシュ
    git push origin "$target_branch"
    push_result=$?

    if [ $push_result -eq 0 ]; then
      echo "リモートに保存しました。最新の状態をpullします..."
      git pull origin "$target_branch"
      pull_result=$?
      if [ $pull_result -ne 0 ]; then
        echo "pull中にコンフリクトが発生しました。処理を中断します。"
        exit 1
      else
        echo "最新の状態に更新しました。"
      fi
    else
      echo "リモートへの保存に失敗しました。pullはスキップします。"
    fi
    ;;
copy)
    echo "現在のブランチ:"
    git branch --list

    read -p "コピー元のブランチ名を入力してください: " source_branch
    # コピー元のブランチが存在するか確認
    if ! git rev-parse --verify --quiet "$source_branch"; then
      echo "エラー: ブランチ '$source_branch' は存在しません。"
      exit 1
    fi

    read -p "新しいブランチ名を入力してください: " new_branch

    read -p "ローカルブランチとしてコピーしますか？ (y/N): " copy_local_confirm
    if [[ "$copy_local_confirm" =~ ^[Yy]$ ]]; then
      git branch "$new_branch" "$source_branch"
      if [ $? -eq 0 ]; then
        echo "ローカルブランチ '$new_branch' を '$source_branch' からコピーしました。"
      else
        echo "エラー: ローカルブランチ '$new_branch' の作成に失敗しました。"
      fi
    else
      read -p "リモートブランチ 'origin/$new_branch' としてコピーしますか？ (y/N): " copy_remote_confirm
      if [[ "$copy_remote_confirm" =~ ^[Yy]$ ]]; then
        current_branch=$(git symbolic-ref --short HEAD)
        git push origin "$source_branch":"refs/heads/$new_branch"
        if [ $? -eq 0 ]; then
          echo "リモートブランチ 'origin/$new_branch' を '$source_branch' からコピーしました。"
        else
          echo "エラー: リモートブランチ 'origin/$new_branch' の作成に失敗しました。"
        fi
      fi
    fi
    ;;
delete)
    echo "削除可能なブランチ:"
    git fetch --prune origin # リモートの最新情報を取得

    git branch -a

    read -p "削除したいブランチ名を選択してください (例: feature-branch, origin/feature-branch): " delete_branch_name

    if [[ "$delete_branch_name" =~ ^origin/ ]]; then
      # リモートブランチの削除 (origin/ブランチ名 形式で指定された場合)
      remote_branch=$(echo "$delete_branch_name" | sed 's|^origin/||')
      read -p "リモートリポジトリ 'origin' のブランチ '$remote_branch' を削除しますか？ (y/N): " confirm_delete_remote
      if [[ "$confirm_delete_remote" =~ ^[Yy]$ ]]; then
        git push origin --delete "$remote_branch"
        if [ $? -eq 0 ]; then
          echo "リモートリポジトリ 'origin' のブランチ '$remote_branch' を削除しました。"
        else
          echo "エラー: リモートリポジトリ 'origin' のブランチ '$remote_branch' の削除に失敗しました。"
        fi
      fi
    else
      # ローカルブランチの削除 (それ以外の形式の場合)
      # 現在のブランチでないことを確認
      current_branch=$(git symbolic-ref --short HEAD 2>/dev/null)
      if [ "$current_branch" = "$delete_branch_name" ]; then
        echo "エラー: 現在チェックアウト中のブランチ '$delete_branch_name' は削除できません。"
        exit 1
      fi

      # ブランチが存在するか確認
      if ! git rev-parse --verify --quiet "refs/heads/$delete_branch_name"; then
        echo "エラー: ローカルブランチ '$delete_branch_name' は存在しません。"
        exit 1
      fi

      git branch -d "$delete_branch_name"
      if [ $? -eq 0 ]; then
        echo "ローカルブランチ '$delete_branch_name' を削除しました。"
      else
        echo "エラー: ローカルブランチ '$delete_branch_name' の削除に失敗しました。"
      fi
    fi
    ;;
  create)
      echo "現在のブランチ:"
      git branch --list
      read -p "ローカルブランチを作成しますか？ (y/N): " create_local

      if [[ "$create_local" =~ ^[Yy]$ ]]; then
        read -p "作成するローカルブランチ名を入力してください: " local_branch_name
        git branch "$local_branch_name"
        if [ $? -eq 0 ]; then
          echo "ローカルブランチ '$local_branch_name' を作成しました。"
        else
          echo "エラー: ローカルブランチ '$local_branch_name' の作成に失敗しました。"
        fi
        echo "現在のブランチ一覧:"
        git branch --list
      elif [[ "$create_local" =~ ^[Nn]$ ]]; then
        read -p "作成するリモートブランチ名を入力してください: " remote_branch_name
        current_branch=$(git symbolic-ref --short HEAD)
        read -p "リモートブランチ '$remote_branch_name' を現在のブランチ '$current_branch' から作成し、プッシュしますか？ (y/N): " confirm_push
        if [[ "$confirm_push" =~ ^[Yy]$ ]]; then
          git push origin "$current_branch":"refs/heads/$remote_branch_name"
          if [ $? -eq 0 ]; then
            echo "リモートブランチ 'origin/$remote_branch_name' を作成し、プッシュしました。"
            read -p "ローカルブランチも '$remote_branch_name' に切り替えますか？ (y/N): " switch_local
            if [[ "$switch_local" =~ ^[Yy]$ ]]; then
              git checkout "$remote_branch_name"
              if [ $? -eq 0 ]; then
                echo "ローカルブランチを '$remote_branch_name' に切り替えました。"
              else
                echo "エラー: ローカルブランチの切り替えに失敗しました。"
              fi
            fi
          else
            echo "エラー: リモートブランチ 'origin/$remote_branch_name' の作成またはプッシュに失敗しました。"
          fi
          echo "現在のブランチ一覧:"
          git branch --list
        fi
      else
        echo "無効な入力です。'y' または 'n' を入力してください。"
        exit 1
      fi
      ;;
  *)

    echo "Usage: $0 {init|remote|add|commit|push|all|setup|branch|checkout|switch|merge|save|delete|create} [arguments]"
    exit 1
    ;;
esac