-- Workbook 3 schema, version 1. Workbook 3 is a fresh design: there is no
-- Workbook 2 compatibility layer.

CREATE TABLE workbooks (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL
) STRICT;

CREATE TABLE tags (
    id   TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('subject', 'grade', 'unit', 'skill', 'other')),
    name TEXT NOT NULL
) STRICT;

CREATE TABLE workbook_tags (
    workbook_id TEXT NOT NULL REFERENCES workbooks(id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES tags(id) ON DELETE RESTRICT,
    PRIMARY KEY (workbook_id, tag_id)
) STRICT;

CREATE TABLE learning_cards (
    id          TEXT PRIMARY KEY,
    workbook_id TEXT NOT NULL REFERENCES workbooks(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    blocks_json TEXT NOT NULL,
    created_at  TEXT NOT NULL
) STRICT;

CREATE INDEX learning_cards_workbook ON learning_cards(workbook_id);

CREATE TABLE questions (
    id                 TEXT PRIMARY KEY,
    workbook_id        TEXT NOT NULL REFERENCES workbooks(id) ON DELETE CASCADE,
    card_id            TEXT REFERENCES learning_cards(id) ON DELETE SET NULL,
    skill_id           TEXT REFERENCES tags(id) ON DELETE SET NULL,
    form               TEXT NOT NULL CHECK (form IN (
                            'single_choice', 'multiple_choice', 'true_false',
                            'short_answer', 'fill_blank', 'flashcard', 'numeric',
                            'ordering', 'matching', 'hotspot', 'labeling')),
    kind               TEXT NOT NULL CHECK (kind IN ('recall', 'skill')),
    difficulty         TEXT NOT NULL CHECK (difficulty IN ('beginner', 'intermediate', 'advanced')),
    prompt_json        TEXT NOT NULL,
    answer_json        TEXT NOT NULL,
    explanation_json   TEXT NOT NULL,
    hint               TEXT,
    tags_json          TEXT NOT NULL DEFAULT '[]',
    estimated_seconds  INTEGER NOT NULL CHECK (estimated_seconds > 0),
    created_at         TEXT NOT NULL
) STRICT;

CREATE INDEX questions_workbook ON questions(workbook_id);
CREATE INDEX questions_skill ON questions(skill_id);

CREATE TABLE groups (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL
) STRICT;

CREATE TABLE group_workbooks (
    group_id    TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    workbook_id TEXT NOT NULL REFERENCES workbooks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (group_id, workbook_id)
) STRICT;

-- Plans are stored from Phase 0 but the Exam/Plan UI is Phase 3.
CREATE TABLE plans (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    target_type   TEXT NOT NULL CHECK (target_type IN ('workbook', 'group', 'all')),
    target_id     TEXT,
    due_date      TEXT,
    daily_minutes INTEGER NOT NULL CHECK (daily_minutes > 0),
    goal          TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL
) STRICT;

CREATE TABLE review_states (
    question_id     TEXT PRIMARY KEY REFERENCES questions(id) ON DELETE CASCADE,
    stability       REAL NOT NULL,
    difficulty      REAL NOT NULL,
    due_at          TEXT NOT NULL,
    reps            INTEGER NOT NULL DEFAULT 0,
    lapses          INTEGER NOT NULL DEFAULT 0,
    last_rating     INTEGER NOT NULL CHECK (last_rating BETWEEN 1 AND 4),
    last_reviewed_at TEXT NOT NULL
) STRICT;

CREATE INDEX review_states_due ON review_states(due_at);

CREATE TABLE skill_states (
    skill_id                TEXT PRIMARY KEY REFERENCES tags(id) ON DELETE CASCADE,
    attempts                INTEGER NOT NULL DEFAULT 0,
    distinct_questions      INTEGER NOT NULL DEFAULT 0,
    first_exposure_attempts INTEGER NOT NULL DEFAULT 0,
    first_exposure_successes INTEGER NOT NULL DEFAULT 0,
    hint_uses               INTEGER NOT NULL DEFAULT 0,
    recent_json             TEXT NOT NULL DEFAULT '[]',
    mastery                 REAL NOT NULL DEFAULT 0,
    updated_at              TEXT NOT NULL
) STRICT;

CREATE TABLE sessions (
    id             TEXT PRIMARY KEY,
    workbook_id    TEXT REFERENCES workbooks(id) ON DELETE SET NULL,
    budget_minutes INTEGER NOT NULL CHECK (budget_minutes > 0),
    status         TEXT NOT NULL CHECK (status IN ('active', 'completed', 'abandoned')),
    started_at     TEXT NOT NULL,
    ended_at       TEXT,
    current_index  INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX sessions_status ON sessions(status);

CREATE TABLE session_items (
    session_id       TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    question_id      TEXT NOT NULL REFERENCES questions(id) ON DELETE RESTRICT,
    position         INTEGER NOT NULL,
    show_card        INTEGER NOT NULL DEFAULT 0,
    card_seen        INTEGER NOT NULL DEFAULT 0,
    answered         INTEGER NOT NULL DEFAULT 0,
    attempt_id       TEXT,
    selection_reason TEXT NOT NULL,
    PRIMARY KEY (session_id, question_id),
    UNIQUE (session_id, position)
) STRICT;

CREATE TABLE attempts (
    attempt_id       TEXT PRIMARY KEY,
    session_id       TEXT NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
    question_id      TEXT NOT NULL REFERENCES questions(id) ON DELETE RESTRICT,
    submitted_at     TEXT NOT NULL,
    response_json    TEXT NOT NULL,
    outcome          TEXT NOT NULL CHECK (outcome IN ('correct', 'partial', 'incorrect', 'ungradable')),
    rating           INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 4),
    score            REAL NOT NULL,
    hint_used        INTEGER NOT NULL DEFAULT 0,
    near_miss_retry  INTEGER NOT NULL DEFAULT 0,
    first_exposure   INTEGER NOT NULL DEFAULT 0,
    elapsed_ms       INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX attempts_question ON attempts(question_id, submitted_at);
CREATE INDEX attempts_session ON attempts(session_id, submitted_at);

CREATE TABLE app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
