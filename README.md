# Workbook 3

AIが教材を作り、人間が効率よく学ぶための、5教科対応のデスクトップ問題集。
Phase 0（足場）と Phase 1（動く学習の縦切り）を実装した v0.1.0 です。

- Rust + GPUI 0.2.2（macOS優先）
- SQLite（ローカル、オフラインファースト）
- Recall は FSRS-6（`fsrs` 6.6.2）、Skill は説明可能な暫定指標
- Learning Card → Question → Feedback → 次へ → 終了／再開
- Phase 1 の回答UI: 単一選択・短答・数値・フラッシュカード

## 起動

```sh
cargo run
```

データは既定で次の場所に保存されます。

```text
~/Library/Application Support/Workbook3/workbook3.sqlite3
```

開発用に保存先を変える場合:

```sh
WORKBOOK3_DATA_DIR="$PWD/.dev-data" cargo run
```

初回起動時、DBが空のときだけアプリ専用DBへ自作サンプル教材（5教科・21問・
5枚のLearning Card）を投入します。任意のディレクトリを削除する処理はありません。

## 操作

- Home: 今日の学習、進行中セッションの再開、3/5/10/20分の時間予算
- Library: Workbook一覧 → Workbook詳細 → Card／Question一覧 → 学習開始
- 学習中: `Enter` 回答／次へ、`1`〜`6` 選択、`F1` ヒント、`Esc` 中断
- フラッシュカード: `Space` で裏面、その後「もう一度／思い出せた／簡単」を自己評価
- 中断しても回答済みの内容は保存され、Homeから再開できます

## 検証

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

テストはUI非依存のコアとSQLiteを対象にしています。GPUI画面は実際に起動し、
Card → 問題 → 採点 → 次へ → 完了、中断・再起動後の再開、Near Miss、
フラッシュカード、数値・選択問題をmacOS上で手動確認済みです。

## 実装状況

### Phase 0 / Phase 1 実装済み

- domain型、11回答形式の判別可能なschema、共通ContentBlock schema
- SQLite schema v1、マイグレーション、Repository、トランザクション
- Attemptの冪等化（Attempt ID）、回答・復習状態・セッションの原子的更新
- FSRS-6 adapter、Recall/Skillの履歴分離、Skillの暫定習得指標
- 決定論的Planner（復習期限、弱点Skill、未学習、時間予算）
- Home / Library / Plans（PlansはPhase 3の説明画面）/ Workbook / Study
- Card初回提示（既読は再表示しない）、セッション中断・再開
- Near Miss（表記ゆれ、丸め、単位不足）と再入力1回
- ヒント利用・補助後正解を自力正解と区別
- 5教科の自作サンプル教材、README、設計ドキュメント

### 未実装（設計のみ）

- Phase 2: 残り7回答形式、Markdown表・数式・画像・Flowchart・Diagram・Graph・Geometryの描画、Hint Overlay、正規化座標の本格検証
- Phase 3: 本格的なSkill習得追跡、Group、タグ範囲Plan、Exam、期限・時間不足警告
- Phase 4: AI教材生成、モデルadapter、構造化検証、プレビュー、部分再生成
- Phase 5: 実教材での品質検証、UI磨き込み、アクセシビリティ監査、クラッシュ復元、性能測定、リリース準備

詳細は [docs/roadmap.md](docs/roadmap.md) を参照してください。

## 設計ドキュメント

- [docs/architecture.md](docs/architecture.md)
- [docs/schema.md](docs/schema.md)
- [docs/roadmap.md](docs/roadmap.md)

## 注意

FSRS-7は今回採用していません。`fsrs-rs` 6.6.2 が提供するFSRS-6を
`RecallScheduler` adapter越しに明示採用しています。FSRS-7実装が安定したら
adapterを差し替える設計です。Skillの習得度はBKT/PFAではなく、
初見正解率（60%）と直近成績（40%）による暫定値です。
