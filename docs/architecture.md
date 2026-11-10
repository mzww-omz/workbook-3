# Architecture

## 目的

Workbook 3 は「教材を作る／解く／履歴を残す／次の出題を決める」ことに絞った
学習寄りの問題集です。学習管理ダッシュボードや多機能エディタにはしません。
Phase 1 は、Home → Workbook → Learning Card → Question → Feedback → 次へ →
終了／再開を、SQLiteに永続化しながら完全に動かすことを目標にしています。

## モジュール境界

単一crate・論理モジュール構成です。crate分割はしていません。

```text
src/
  domain.rs      教材・回答・履歴・セッション・Planの型
  content.rs     CardとQuestionが共有する宣言的ContentBlock
  study/
    grading.rs   決定的な採点（FSRS評価とは分離）
    fsrs.rs      Recall用FSRS adapter
    skill.rs     Skill用の説明可能な暫定estimator
    planner.rs   唯一の決定論的Planner
  storage/       SQLite schema・migration・Repository
  seed.rs        5教科の自作サンプル教材
  ui/            GPUI画面・単一行テキスト入力・共通スタイル
```

UIは採点・スケジューリングを実装しません。UIは入力と結果表示だけを担当し、
`grade` → `Repository::submit_attempt` の順に呼びます。

## 学習フロー

```text
Home / Workbook
  └─ start_session(workbook?, budget)
       ├─ Planner: 復習期限 → 弱点Skill → 未学習 → 追加練習
       ├─ sessions + session_items を保存（開始時に問題を確定）
       └─ Study
            ├─ 初回だけ LearningCard
            ├─ Question
            ├─ grade
            │    ├─ NearMiss: 保存せず1回だけ再入力
            │    └─ Final: submit_attempt（原子的）
            ├─ Feedback（同じ文脈に解説）
            └─ 次へ / 終了（中断）
```

- Cardを読んだだけではRecallの記憶状態を更新しません。回答して初めて更新します。
- 未回答の問題は正解・不正解のどちらにも数えません。時間切れで自動提出もしません。
- 初回Cardは `session_items.card_seen` で管理し、再開時に既読Cardを繰り返しません。

## Recall と Skill

`AnswerForm`（回答UI）と `LearningKind`（学習目的）は独立です。

- Recall: `review_states` にFSRSのstability/difficulty/dueを持つ。回答評価から
  Again/Hard/Good/Easyを導出する。ヒント利用・Near Miss再入力後の正解はAgain。
- Skill: `skill_states` をSkill ID単位で更新する。同じ問題の再回答は
  初見正解率を上げない。`SkillEstimator` traitでアルゴリズム交換余地を残す。
- 現在のSkill習得度 = 初見正解率 × 0.6 + 直近8回の重み付き成績 × 0.4。
  補助あり正解は0.5、部分点は0.25×score、誤答は0。説明可能な暫定値であり、
  BKT/PFAを検証なしに実装したものではない。

## FSRS

`study::fsrs::RecallScheduler` が境界です。現在の実装は
`Fsrs6Scheduler` で、`fsrs-rs` 6.6.2 のFSRS-6（21パラメータ）を利用します。
FSRS-7は採用していません。due日時はExam都合で書き換えません（ExamはPhase 3）。

- Againは10分〜1日の再学習、その他はFSRSのintervalをそのまま使う。
- 保存するのはstability/difficulty/due/reps/lapses/last rating/last reviewed at。
- `submit_attempt` 内でattempt挿入とreview_states更新を同一トランザクションにする。

## Planner

`study::planner::plan_session` は純粋関数です。入力は候補問題、時間予算、現在時刻。
優先順は次のとおりです。

1. dueを過ぎたRecall（超過時間が大きい順）
2. 習得度60%未満のSkill（低い順）
3. 未学習
4. 追加練習

初回Cardがある問題はCard読解時間（30秒）を予算に加えます。予算に収まる範囲で
選び、1問も収まらない場合は最低1問を選んで `over_budget` を返します。
選択理由は `session_items.selection_reason` に保存し、後から説明できます。

## 永続化と冪等性

- `attempts` は追記型。主キーはクライアント生成の `attempt_id`。
- 同じ `attempt_id` の再送は `Duplicate`。回答済みセッション項目への別ID送信は
  `AlreadyAnswered`。どちらも件数を増やさない。
- attempt挿入、review/skill更新、session item answered、current_index更新を
  1トランザクションで行う。テストでは挿入後の故障注入でロールバックを確認。
- セッション開始時に問題を確定する。Workbookへ後から追加された問題は
  次のセッションから反映される。

## UI

- GPUI 0.2.2。単一行入力はIME対応の `EntityInputHandler` を実装。
- 画面は Home / Library / Plans / Workbook詳細 / Study。
- Studyは「終了・単元名・進捗・問題・回答・ヒント・次へ」に限定。
- 白・薄いグレー・ほぼ黒＋青1色。角丸・グラデーション・ガラス効果は使わない。
- キーボード: Enter、1〜6、⌘H、Space、Esc。入力欄は問題表示時にフォーカス。

## Phase 2以降の拡張点

- `ContentBlock` にTable/Math/Image/Flowchart/Diagram/Graph/Geometryを定義済み。
  描画と検証をPhase 2で追加する。
- `AnswerSpec` に11形式を定義済み。Phase 1 UIは4形式。
- `Plan`/`groups`/`group_workbooks` はschemaのみ。Phase 3でPlannerに接続。
- 将来の `generation` は `Repository` と検証済みContentBlockを介して保存する。
  任意HTML/JSは実行しない。
