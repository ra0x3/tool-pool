-- Create todos table
CREATE TABLE IF NOT EXISTS todos (
    id VARCHAR(50) PRIMARY KEY,
    user_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    completed BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE
);

-- Create WAL table for audit log
CREATE TABLE IF NOT EXISTS wal_entries (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    operation VARCHAR(50) NOT NULL,
    data JSONB NOT NULL
);

-- Create indexes for better performance
CREATE INDEX idx_todos_user_id ON todos(user_id);
CREATE INDEX idx_todos_completed ON todos(completed);
CREATE INDEX idx_wal_timestamp ON wal_entries(timestamp);

-- Insert some initial data
INSERT INTO todos (id, user_id, title, completed, created_at) VALUES
    ('todo-1', 1, 'Setup PostgreSQL database', true, NOW()),
    ('todo-2', 1, 'Learn WASI and MCP', false, NOW()),
    ('todo-3', 1, 'Build full-stack application', false, NOW()),
    ('todo-4', 2, 'Test database operations', false, NOW())
ON CONFLICT DO NOTHING;

-- Create a view for todo statistics
CREATE OR REPLACE VIEW todo_stats AS
SELECT
    COUNT(*) as total,
    COUNT(CASE WHEN completed THEN 1 END) as completed,
    COUNT(CASE WHEN NOT completed THEN 1 END) as pending,
    COUNT(DISTINCT user_id) as unique_users
FROM todos;