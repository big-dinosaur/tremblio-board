-- 用户
CREATE TABLE bbs_users (
    id            BIGINT(20)   NOT NULL AUTO_INCREMENT,
    username      VARCHAR(32)  NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    nickname      VARCHAR(64)  NOT NULL DEFAULT '',
    is_admin      TINYINT      NOT NULL DEFAULT 0,
    is_banned     TINYINT      NOT NULL DEFAULT 0,
    created_at    BIGINT(20)   NOT NULL,
    updated_at    BIGINT(20)   NOT NULL,
    last_login_at BIGINT(20)   NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    UNIQUE KEY uk_bbs_users_username (username)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 登录 token（只存 SHA-256 摘要）
CREATE TABLE bbs_tokens (
    id           BIGINT(20) NOT NULL AUTO_INCREMENT,
    user_id      BIGINT(20) NOT NULL,
    token_hash   CHAR(64)   NOT NULL,
    created_at   BIGINT(20) NOT NULL,
    expires_at   BIGINT(20) NOT NULL,
    last_used_at BIGINT(20) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_bbs_tokens_hash (token_hash),
    KEY idx_bbs_tokens_user (user_id),
    CONSTRAINT fk_bbs_tokens_user FOREIGN KEY (user_id) REFERENCES bbs_users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 板块
CREATE TABLE bbs_boards (
    id           BIGINT(20)   NOT NULL AUTO_INCREMENT,
    name         VARCHAR(64)  NOT NULL,
    description  VARCHAR(500) NOT NULL DEFAULT '',
    sort_order   BIGINT(20)   NOT NULL DEFAULT 0,
    thread_count BIGINT(20)   NOT NULL DEFAULT 0,
    post_count   BIGINT(20)   NOT NULL DEFAULT 0,
    created_at   BIGINT(20)   NOT NULL,
    updated_at   BIGINT(20)   NOT NULL,
    PRIMARY KEY (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 话题
CREATE TABLE bbs_threads (
    id           BIGINT(20)   NOT NULL AUTO_INCREMENT,
    board_id     BIGINT(20)   NOT NULL,
    user_id      BIGINT(20)   NOT NULL,
    title        VARCHAR(200) NOT NULL,
    reply_count  BIGINT(20)   NOT NULL DEFAULT 0,
    view_count   BIGINT(20)   NOT NULL DEFAULT 0,
    is_pinned    TINYINT      NOT NULL DEFAULT 0,
    is_locked    TINYINT      NOT NULL DEFAULT 0,
    created_at   BIGINT(20)   NOT NULL,
    updated_at   BIGINT(20)   NOT NULL,
    last_post_at BIGINT(20)   NOT NULL,
    PRIMARY KEY (id),
    KEY idx_bbs_threads_board (board_id, is_pinned, last_post_at),
    KEY idx_bbs_threads_user (user_id),
    CONSTRAINT fk_bbs_threads_board FOREIGN KEY (board_id) REFERENCES bbs_boards (id) ON DELETE CASCADE,
    CONSTRAINT fk_bbs_threads_user FOREIGN KEY (user_id) REFERENCES bbs_users (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- 帖子
CREATE TABLE bbs_posts (
    id         BIGINT(20) NOT NULL AUTO_INCREMENT,
    thread_id  BIGINT(20) NOT NULL,
    user_id    BIGINT(20) NOT NULL,
    content    MEDIUMTEXT NOT NULL,
    created_at BIGINT(20) NOT NULL,
    updated_at BIGINT(20) NOT NULL,
    PRIMARY KEY (id),
    KEY idx_bbs_posts_thread (thread_id, id),
    KEY idx_bbs_posts_user (user_id),
    CONSTRAINT fk_bbs_posts_thread FOREIGN KEY (thread_id) REFERENCES bbs_threads (id) ON DELETE CASCADE,
    CONSTRAINT fk_bbs_posts_user FOREIGN KEY (user_id) REFERENCES bbs_users (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
