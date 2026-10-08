# tremblio-board

基于 Rust（axum + sqlx）的简易 BBS 后端，提供 JSON REST API，同时支持 MySQL 与 SQLite。

结构：板块（board）→ 话题（thread）→ 帖子（post），另有用户与登录 token。

## 运行

```bash
cp .env.example .env   # 按需修改 DATABASE_URL
cargo run
```

启动时会根据 `DATABASE_URL` 前缀（`mysql://` / `sqlite://`）自动执行 `migrations/mysql` 或 `migrations/sqlite` 下的迁移。

## 数据表

| 表 | 说明 |
|---|---|
| `bbs_users` | 用户，密码以 argon2 哈希保存 |
| `bbs_tokens` | 登录 token，只保存 SHA-256 摘要 |
| `bbs_boards` | 板块，冗余 `thread_count` / `post_count` |
| `bbs_threads` | 话题，冗余 `reply_count`、`last_post_at` |
| `bbs_posts` | 帖子，每个话题的第一条为首帖 |

主键在 MySQL 中为 `BIGINT(20) AUTO_INCREMENT`；SQLite 只有 `INTEGER PRIMARY KEY` 能自增，其本身就是 64 位整数。时间字段均为 Unix 秒级时间戳。

## 权限

- 第一个注册的用户自动成为管理员
- 浏览无需登录；发话题、回复需要登录（`Authorization: Bearer <token>`）
- 板块增删改、话题置顶/锁定：仅管理员
- 修改/删除话题或帖子：作者本人或管理员
- 锁定的话题只有管理员能回复；首帖不能单独删除，需删除整个话题

## API

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/auth/register` | 注册 `{username, password, nickname?}` |
| POST | `/api/auth/login` | 登录 `{username, password}` → `{token, expires_at, user}` |
| POST | `/api/auth/logout` | 登出（使当前 token 失效） |
| GET | `/api/auth/me` | 当前用户 |
| GET / POST | `/api/boards` | 板块列表 / 创建 `{name, description?, sort_order?}` |
| GET / PUT / DELETE | `/api/boards/{id}` | 板块详情 / 修改 / 删除（级联删除话题与帖子） |
| GET / POST | `/api/boards/{id}/threads` | 话题列表（置顶优先、按最后回复倒序）/ 发话题 `{title, content}` |
| GET / PUT / DELETE | `/api/threads/{id}` | 话题详情（浏览数 +1）/ 修改 `{title?, is_pinned?, is_locked?}` / 删除 |
| GET / POST | `/api/threads/{id}/posts` | 帖子列表 / 回复 `{content}` |
| GET / PUT / DELETE | `/api/posts/{id}` | 帖子详情 / 修改 `{content}` / 删除 |

列表接口支持 `?page=1&per_page=20`（`per_page` 上限 100），返回 `{items, total, page, per_page}`。

错误统一返回 `{"error": "..."}` 及对应 HTTP 状态码。
