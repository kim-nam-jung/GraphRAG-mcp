package storage

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"

	sqlite_vec "github.com/asg017/sqlite-vec-go-bindings/cgo"
	_ "github.com/mattn/go-sqlite3"
)

const SchemaSQL = `
-- 엔티티 (노드)
CREATE TABLE IF NOT EXISTS entities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    type TEXT NOT NULL,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER,
    language TEXT,
    community_id INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(file_path, name, line_start)
);
CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(type);
CREATE INDEX IF NOT EXISTS idx_entities_community ON entities(community_id);
CREATE INDEX IF NOT EXISTS idx_entities_file ON entities(file_path);
CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);

-- 관계 (엣지)
CREATE TABLE IF NOT EXISTS relations (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    type TEXT NOT NULL,
    weight REAL DEFAULT 1.0,
    UNIQUE(source_id, target_id, type)
);
CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id);
CREATE INDEX IF NOT EXISTS idx_relations_type ON relations(type);

-- 코드 청크
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    language TEXT,
    entity_id INTEGER REFERENCES entities(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
CREATE INDEX IF NOT EXISTS idx_chunks_entity ON chunks(entity_id);

-- 커뮤니티
CREATE TABLE IF NOT EXISTS communities (
    id INTEGER PRIMARY KEY,
    level INTEGER NOT NULL,
    parent_community_id INTEGER REFERENCES communities(id)
);

-- 파일 해시 (증분 인덱싱)
CREATE TABLE IF NOT EXISTS file_hashes (
    file_path TEXT PRIMARY KEY,
    hash TEXT NOT NULL,
    mtime INTEGER NOT NULL,
    size INTEGER NOT NULL
);

-- FTS5
CREATE VIRTUAL TABLE IF NOT EXISTS fts_chunks USING fts5(
    text, file_path, content=chunks, content_rowid=id
);
CREATE VIRTUAL TABLE IF NOT EXISTS fts_entities USING fts5(
    name, qualified_name, content=entities, content_rowid=id
);

-- FTS5 동기화 트리거
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO fts_chunks(rowid, text, file_path) VALUES (new.id, new.text, new.file_path);
END;
CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO fts_chunks(fts_chunks, rowid, text, file_path) VALUES ('delete', old.id, old.text, old.file_path);
END;
CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO fts_chunks(fts_chunks, rowid, text, file_path) VALUES ('delete', old.id, old.text, old.file_path);
    INSERT INTO fts_chunks(rowid, text, file_path) VALUES (new.id, new.text, new.file_path);
END;

CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO fts_entities(rowid, name, qualified_name) VALUES (new.id, new.name, new.qualified_name);
END;
CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
    INSERT INTO fts_entities(fts_entities, rowid, name, qualified_name) VALUES ('delete', old.id, old.name, old.qualified_name);
END;
CREATE TRIGGER IF NOT EXISTS entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO fts_entities(fts_entities, rowid, name, qualified_name) VALUES ('delete', old.id, old.name, old.qualified_name);
    INSERT INTO fts_entities(rowid, name, qualified_name) VALUES (new.id, new.name, new.qualified_name);
END;

-- sqlite-vec (Harrier 640차원)
CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding float[640]
);

-- 메타데이터
CREATE TABLE IF NOT EXISTS index_meta (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL,
    languages JSON,
    total_files INTEGER,
    total_entities INTEGER,
    total_relations INTEGER,
    total_chunks INTEGER,
    total_communities INTEGER,
    indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
`

func init() {
	// Register sqlite3 automatic load with extensions
	// sqlite-vec handles the automatic loading for the standard driver
	sqlite_vec.Auto()
}

// Database represents a wrapper around our GraphRAG sqlite db
type Database struct {
	DB *sql.DB
}

// InitDB initializes the sqlite database with PRAGMAS and the base schema
func InitDB(dbPath string, walMode bool) (*Database, error) {
	if dbPath == ":memory:" {
		dbPath = fmt.Sprintf("file:memdb%x?mode=memory&cache=shared", os.Getpid())
	} else {
		err := os.MkdirAll(filepath.Dir(dbPath), 0755)
		if err != nil {
			return nil, fmt.Errorf("failed to create data dir: %w", err)
		}
	}

	// We use "sqlite3" which is auto-registered by sqlite_vec.Auto()
	// and supports foreign keys enabled via PRAGMA
	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}

	// Set recommended pragmas for performance and concurrency
	pragmas := []string{
		"PRAGMA foreign_keys = ON",
		"PRAGMA synchronous = NORMAL",
		"PRAGMA temp_store = MEMORY",
		"PRAGMA mmap_size = 30000000000",
		"PRAGMA page_size = 32768",
	}

	if walMode {
		pragmas = append(pragmas, "PRAGMA journal_mode = WAL")
	} else {
		pragmas = append(pragmas, "PRAGMA journal_mode = DELETE")
	}

	for _, p := range pragmas {
		if _, err := db.Exec(p); err != nil {
			return nil, fmt.Errorf("execute pragma %s failed: %w", p, err)
		}
	}

	// Apply schema
	if _, err := db.Exec(SchemaSQL); err != nil {
		return nil, fmt.Errorf("failed to apply database schema: %w", err)
	}

	return &Database{
		DB: db,
	}, nil
}

// Close safely shuts down the database connection
func (d *Database) Close() error {
	if d.DB != nil {
		return d.DB.Close()
	}
	return nil
}
