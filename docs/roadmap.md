# Roadmap

今回の実装範囲は **Phase 0 と Phase 1** です。Phase 2以降には自動着手していません。
各フェーズの着手条件と受け入れ条件を短く記録します。

## Phase 0 — 仕様と足場（完了）

- Rust 1.96 / macOS / GPUI 0.2.2 / SQLite bundled のビルド確認
- domain型、11回答形式、共通ContentBlock schema
- schema v1、migration、Repository、トランザクション
- FSRS adapter、Skill estimator、Plannerの純粋関数
- 実行可能なGPUIウィンドウ、サンプル教材
- `docs/architecture.md`, `docs/schema.md`, `docs/roadmap.md`

受け入れ: `cargo build` / `cargo test` が通り、空DBから起動できること。

## Phase 1 — 学習の縦切り（完了）

- Home → Workbook → LearningCard → Question → Feedback → 次へ → 終了／再開
- 単一選択・短答・数値・フラッシュカードのUI
- FSRS-6によるRecall、Attempt履歴、ReviewStateの永続化
- Recall/Skill分類、Skill ID記録、既知問題と初見の区別
- 3/5/10/20分のPlanner、選択理由の保存
- Near Miss（表記・丸め・単位）、ヒント／補助後正解の区別
- 中断・再開、SQLite再起動後の復元
- 5教科の自作サンプル

受け入れ: UIを実際に起動し、Card→回答→Feedback→完了と中断→再開を確認できること。

## Phase 2 — 表現力と残り7形式

対象:

- multiple_choice / true_false / fill_blank / ordering / matching / hotspot / labeling
- Markdown表、数式、画像、Flowchart、Diagram、Graph、Geometry
- 共通Hint Overlay、0〜1正規化座標、SVG/画像の検証・無害化
- Geometryの数学的関係と表示座標の分離、矛盾検証

着手条件:

- Phase 1の採点インターフェースを維持したまま、回答形式ごとのrendererとgraderを追加できる
- 画像資産の保存先と検証方針を決める
- 11形式すべてのサンプルと採点テストを用意する

非目標: フルCAD、一般制約ソルバ、任意HTML/JS実行。

## Phase 3 — Planner / Exam / Skill改善

対象:

- Group、共通タグ、Workbook/Group/タグ範囲のPlan
- 期限、1日の時間予算、時間不足の明示（「必ず間に合う」と偽らない）
- Exam日までの日数と弱点をPlannerの優先度に追加
- Skillの初見バリエーション、交互学習、習得追跡の改善
- Plan対象への問題追加の次セッション反映

制約:

- FSRSのdueをExam都合で改ざんしない
- Exam中の回答も通常のattemptsへ追記する
- 高度な知識追跡は検証可能な指標から段階的に導入する

## Phase 4 — AI教材生成

対象:

- OpenAI互換API／ローカルモデルの交換可能なadapter
- Workbook/カード/問題/ヒント/解説/タグ/図形構造の構造化生成
- スキーマ検証、数学・図形・回答仕様の決定的検証、プレビュー、部分再生成
- APIキーをログ・DB平文に残さない

制約:

- 生成結果からコードを実行しない
- 未検証問題は未検証として保存・表示する
- オフラインでも保存済み教材を学習できる

## Phase 5 — 品質向上

対象:

- 5教科の実教材に近いデータでの検証
- UI磨き込み、アクセシビリティ（フォーカス、読み上げ、キーボード）
- クラッシュ復元、バックアップ、パフォーマンス、採点精度
- リリースビルド、macOS配布準備

受け入れ:

- 代表的な教材セットで、復習・Skill・Near Miss・再開の回帰テスト
- 起動・学習・終了のクラッシュ復元テスト
- 実データ規模のクエリと描画時間の計測
