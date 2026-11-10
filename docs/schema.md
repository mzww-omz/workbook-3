# SQLite schema

Workbook 3 内だけでバージョン管理します。Workbook 2との互換・移行はありません。
現在は schema version 1（`schema_migrations` と `PRAGMA user_version = 1`）。

## 主要テーブル

| table | 役割 |
|---|---|
| `workbooks` | 教材コンテナ |
| `learning_cards` | 説明用Card。blocks_jsonにContentBlock配列 |
| `questions` | 問題。form/kind/difficulty、prompt/answer/explanation JSON |
| `tags` / `workbook_tags` | 安定IDの分類。subject/grade/unit/skill/other |
| `groups` / `group_workbooks` | Workbook集合（Phase 3でUI接続） |
| `plans` | 対象・期限・1日の時間予算（Phase 3でUI接続） |
| `review_states` | RecallのFSRS記憶状態 |
| `skill_states` | Skill単位の試行数・初見成績・直近成績・習得度 |
| `sessions` | 学習セッション。status/current_index |
| `session_items` | 開始時に確定した問題列。Card既読・回答済み・Attempt ID |
| `attempts` | 追記型の回答履歴。attempt_idが主キー |
| `app_meta` | サンプル投入バージョンなどのメタ情報 |

## 不変条件

- `questions.form` は11形式のいずれか。
- `questions.kind` は `recall` / `skill`。
- `skill` 問題は `skill_id` を持つ（Repositoryが保存時に検証）。
- `review_states` はRecall問題のみ。Skill問題の回答では更新しない。
- `skill_states` はSkill問題のみ。Recall問題の回答では更新しない。
- `attempts` は削除・上書きしない。修正が必要なら新しいAttemptを追記する。
- `session_items.answered` が1の項目へ別Attemptは記録できない。
- `review_states.due_at` はFSRSだけが決める。Exam都合で書き換えない。

## 回答保存のトランザクション

`Repository::submit_attempt` は次を1トランザクションで行います。

1. `attempt_id` の重複確認（あれば `Duplicate` を返し、書込みなし）
2. セッション項目の回答済み確認（あれば `AlreadyAnswered`）
3. `first_exposure` を、その問題の過去Attempt件数から判定
4. `attempts` へ追記
5. Recallなら `review_states` をupsert、Skillなら `skill_states` をupsert
6. `session_items.answered` / `attempt_id` 更新
7. `sessions.current_index` を進める

途中でエラーが起きるとトランザクションはロールバックされます。テストでは
Attempt挿入直後に故障を注入し、Attempt・復習状態・セッションがすべて元に戻ることを
確認しています。

## Near Miss

Near Missは最終回答ではありません。1回目の送信ではDBへ書かず、UIにメッセージを
返します。同じ `attempt_id` のまま再入力し、2回目を最終回答として保存します。
`near_miss_retry = 1` が立ち、正解でも自力正解として扱いません。

## JSON

- `learning_cards.blocks_json`: `Vec<ContentBlock>`
- `questions.prompt_json`, `explanation_json`: `Vec<ContentBlock>`
- `questions.answer_json`: `AnswerSpec`（`type` タグ付き）
- `questions.tags_json`: タグID文字列配列（安定ID。表示名を同一性に使わない）
- `attempts.response_json`: 回答データ
- `skill_states.recent_json`: 直近8回の成功重み配列

ContentBlockとAnswerSpecは宣言的データで、HTML/JavaScript文字列を実行しません。

## 日時

UTCのRFC 3339（ミリ秒、`Z`）で保存します。表示時にローカル時刻へ変換します。

## マイグレーション

`Repository::open` 時に `schema_migrations` を確認し、未適用のv1をトランザクションで
適用します。既存DBのversionが実装より新しい場合は起動を拒否します。
サンプル教材は `app_meta.sample_seed_version` で冪等化し、既存Workbookがある場合は
投入しません。

## インデックス

- `learning_cards_workbook`
- `questions_workbook`, `questions_skill`
- `review_states_due`
- `sessions_status`
- `attempts_question`, `attempts_session`

主要な読み取りはWorkbook詳細、復習期限、セッション再開です。Phase 1のデータ量では
これで十分ですが、Phase 5で実教材規模のEXPLAIN QUERY PLANを確認します。
