-- 初始化用户表
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    nickname VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    role VARCHAR(50) NOT NULL DEFAULT 'member',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 评论表
CREATE TABLE IF NOT EXISTS comments (
    id SERIAL PRIMARY KEY,
    blog_id VARCHAR(255) NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 初始化用户身份关联表 (OAuth)
CREATE TABLE IF NOT EXISTS user_identities (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider VARCHAR(50) NOT NULL,
    provider_uid VARCHAR(255) NOT NULL,
    access_token TEXT,
    refresh_token TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, provider_uid)
);

-- 课程学习进度（lesson 粒度）
CREATE TABLE IF NOT EXISTS course_progress (
    user_id          INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    course_slug      VARCHAR(255) NOT NULL,
    lesson_path      TEXT NOT NULL,           -- '<chapter>/<lesson>'
    completed        BOOLEAN NOT NULL DEFAULT FALSE,
    position_seconds INTEGER,                 -- 视频/音频续播位置
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, course_slug, lesson_path)
);
CREATE INDEX IF NOT EXISTS idx_course_progress_user ON course_progress(user_id);

-- 用户标注（适用于 course / doc / blog）
CREATE TABLE IF NOT EXISTS annotations (
    id            BIGSERIAL PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resource_kind VARCHAR(32) NOT NULL,        -- 'course' | 'doc' | 'blog'
    resource_path TEXT NOT NULL,               -- 叶子页路径
    block_id      VARCHAR(64) NOT NULL,        -- Markdown 顶层块 id
    start_offset  INTEGER NOT NULL,            -- 块内字符偏移
    end_offset    INTEGER NOT NULL,
    exact_text    TEXT NOT NULL,               -- 选中文本快照
    prefix_text   TEXT,                        -- 前 32 字符上下文
    suffix_text   TEXT,                        -- 后 32 字符上下文
    style         VARCHAR(32) NOT NULL,        -- yellow|green|blue|pink|purple|underline|wavy|strikethrough
    note          TEXT,
    visibility    VARCHAR(16) NOT NULL DEFAULT 'private',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_annotations_resource ON annotations(resource_kind, resource_path);
CREATE INDEX IF NOT EXISTS idx_annotations_user ON annotations(user_id);
