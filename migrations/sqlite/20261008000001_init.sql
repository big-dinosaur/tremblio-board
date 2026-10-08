-- SQLite 中只有 "INTEGER PRIMARY KEY" 才能自增，它本身就是 64 位有符号整数（等价 BIGINT）

CREATE TABLE bbs_users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      VARCHAR(32)  NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    nickname      VARCHAR(64)  NOT NULL DEFAULT '',
    is_admin      INTEGER      NOT NULL DEFAULT 0,
    is_banned     INTEGER      NOT NULL DEFAULT 0,
    created_at    BIGINT       NOT NULL,
    updated_at    BIGINT       NOT NULL,
    last_login_at BIGINT       NOT NULL DEFAULT 0
);

CREATE TABLE bbs_tokens (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      BIGINT   NOT NULL REFERENCES bbs_users (id) ON DELETE CASCADE,
    token_hash   CHAR(64) NOT NULL UNIQUE,
    created_at   BIGINT   NOT NULL,
    expires_at   BIGINT   NOT NULL,
    last_used_at BIGINT   NOT NULL
);
CREATE INDEX idx_bbs_tokens_user ON bbs_tokens (user_id);

CREATE TABLE bbs_boards (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         VARCHAR(64)  NOT NULL,
    description  VARCHAR(500) NOT NULL DEFAULT '',
    sort_order   BIGINT       NOT NULL DEFAULT 0,
    thread_count BIGINT       NOT NULL DEFAULT 0,
    post_count   BIGINT       NOT NULL DEFAULT 0,
    created_at   BIGINT       NOT NULL,
    updated_at   BIGINT       NOT NULL
);

CREATE TABLE bbs_threads (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id     BIGINT       NOT NULL REFERENCES bbs_boards (id) ON DELETE CASCADE,
    user_id      BIGINT       NOT NULL REFERENCES bbs_users (id),
    title        VARCHAR(200) NOT NULL,
    reply_count  BIGINT       NOT NULL DEFAULT 0,
    view_count   BIGINT       NOT NULL DEFAULT 0,
    is_pinned    INTEGER      NOT NULL DEFAULT 0,
    is_locked    INTEGER      NOT NULL DEFAULT 0,
    created_at   BIGINT       NOT NULL,
    updated_at   BIGINT       NOT NULL,
    last_post_at BIGINT       NOT NULL
);
CREATE INDEX idx_bbs_threads_board ON bbs_threads (board_id, is_pinned, last_post_at);
CREATE INDEX idx_bbs_threads_user ON bbs_threads (user_id);

CREATE TABLE bbs_posts (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id  BIGINT NOT NULL REFERENCES bbs_threads (id) ON DELETE CASCADE,
    user_id    BIGINT NOT NULL REFERENCES bbs_users (id),
    content    TEXT   NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE INDEX idx_bbs_posts_thread ON bbs_posts (thread_id, id);
CREATE INDEX idx_bbs_posts_user ON bbs_posts (user_id);
