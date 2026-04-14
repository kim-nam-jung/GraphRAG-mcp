use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use tracing::{info, warn};
use serde::Serialize;

use std::sync::Once;

static INIT_SQLITE_VEC: Once = Once::new();

fn init_sqlite_vec() {
    INIT_SQLITE_VEC.call_once(|| {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())));
        }
    });
}

pub struct Database {
    pub conn: Connection,
}

#[derive(Debug, Serialize)]
pub struct EntityRecord {
    pub id: i64,
    pub file_path: String,
    pub name: String,
    pub entity_type: String,
    pub qualified_name: String,
    pub community_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RelationRecord {
    pub source_name: String,
    pub target_name: String,
    pub relation_type: String,
    pub weight: f64,
}

#[derive(Debug, Serialize)]
pub struct EntityDetail {
    pub entity: EntityRecord,
    pub incoming: Vec<RelationRecord>,
    pub outgoing: Vec<RelationRecord>,
}

#[derive(Debug, Serialize)]
pub struct KeywordResult {
    pub name: String,
    pub entity_type: String,
    pub file_path: String,
    pub snippet: String,
}

impl Database {
    pub fn new(db_path: &str, wal_mode: bool) -> Result<Self> {
        init_sqlite_vec();
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database at {}", db_path))?;



        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        if wal_mode {
            conn.execute_batch("
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA temp_store = MEMORY;
                PRAGMA mmap_size = 30000000000;
                PRAGMA page_size = 32768;
            ")?;
            info!("Database configured with WAL mode");
        }

        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                qualified_name TEXT NOT NULL,
                type TEXT NOT NULL,
                file_path TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                community_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS relations (
                source_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                PRIMARY KEY (source_id, target_id, type),
                FOREIGN KEY (source_id) REFERENCES entities(id) ON DELETE CASCADE,
                FOREIGN KEY (target_id) REFERENCES entities(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                file_path TEXT NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                entity_id INTEGER,
                FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS communities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                level INTEGER NOT NULL DEFAULT 0,
                parent_community_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS file_hashes (
                file_path TEXT PRIMARY KEY,
                mtime INTEGER,
                size INTEGER,
                hash TEXT
            );

            CREATE TABLE IF NOT EXISTS index_meta (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                root_path TEXT NOT NULL,
                indexed_at TEXT DEFAULT (datetime('now'))
            );

            -- FTS5 virtual tables
            CREATE VIRTUAL TABLE IF NOT EXISTS fts_chunks USING fts5(
                text,
                content='chunks',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 1'
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS fts_entities USING fts5(
                name,
                qualified_name,
                content='entities',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 1'
            );

            -- FTS sync triggers
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO fts_chunks(rowid, text) VALUES (new.id, new.text);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO fts_chunks(fts_chunks, rowid, text) VALUES('delete', old.id, old.text);
            END;
            CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
                INSERT INTO fts_entities(rowid, name, qualified_name) VALUES (new.id, new.name, new.qualified_name);
            END;
            CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
                INSERT INTO fts_entities(fts_entities, rowid, name, qualified_name) VALUES('delete', old.id, old.name, old.qualified_name);
            END;

            -- Vector embeddings table (sqlite-vec)
            CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding float[640]
            );

            -- Performance indices
            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(type);
            CREATE INDEX IF NOT EXISTS idx_entities_file ON entities(file_path);
            CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);
            CREATE INDEX IF NOT EXISTS idx_entities_community ON entities(community_id);
            CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id);
            CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_entity ON chunks(entity_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
        ").with_context(|| "Failed to initialize database schema")?;

        Ok(())
    }

    // --- Entity Operations ---

    pub fn insert_entity(&self, file_path: &str, name: &str, entity_type: &str, qualified_name: &str) -> Result<i64> {
        let id = self.conn.query_row(
            "INSERT INTO entities (file_path, name, type, qualified_name) VALUES (?1, ?2, ?3, ?4) RETURNING id",
            params![file_path, name, entity_type, qualified_name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn get_entity(&self, name: &str, file: &str) -> Result<Option<EntityDetail>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, name, type, qualified_name, community_id FROM entities WHERE name = ?1 AND file_path = ?2 LIMIT 1"
        )?;

        let entity = stmt.query_row(params![name, file], |row| {
            Ok(EntityRecord {
                id: row.get(0)?,
                file_path: row.get(1)?,
                name: row.get(2)?,
                entity_type: row.get(3)?,
                qualified_name: row.get(4)?,
                community_id: row.get(5)?,
            })
        });

        let entity = match entity {
            Ok(e) => e,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let incoming = self.get_relations_incoming(entity.id)?;
        let outgoing = self.get_relations_outgoing(entity.id)?;

        Ok(Some(EntityDetail { entity, incoming, outgoing }))
    }

    fn get_relations_incoming(&self, entity_id: i64) -> Result<Vec<RelationRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.name, r.type, r.weight FROM relations r JOIN entities e ON r.source_id = e.id WHERE r.target_id = ?1"
        )?;
        let rows = stmt.query_map(params![entity_id], |row| {
            Ok(RelationRecord {
                source_name: row.get(0)?,
                target_name: String::new(),
                relation_type: row.get(1)?,
                weight: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    fn get_relations_outgoing(&self, entity_id: i64) -> Result<Vec<RelationRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.name, r.type, r.weight FROM relations r JOIN entities e ON r.target_id = e.id WHERE r.source_id = ?1"
        )?;
        let rows = stmt.query_map(params![entity_id], |row| {
            Ok(RelationRecord {
                source_name: String::new(),
                target_name: row.get(0)?,
                relation_type: row.get(1)?,
                weight: row.get(2)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // --- Relation Operations ---

    pub fn insert_relation(&self, source_id: i64, target_id: i64, rel_type: &str, weight: f64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO relations (source_id, target_id, type, weight) VALUES (?1, ?2, ?3, ?4)",
            params![source_id, target_id, rel_type, weight],
        )?;
        Ok(())
    }

    // --- Chunk Operations ---

    pub fn insert_chunk(&self, text: &str, file_path: &str, line_start: Option<i64>, line_end: Option<i64>, entity_id: Option<i64>) -> Result<i64> {
        let id = self.conn.query_row(
            "INSERT INTO chunks (text, file_path, line_start, line_end, entity_id) VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
            params![text, file_path, line_start, line_end, entity_id],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    // --- Vector Operations ---

    pub fn write_chunk_embedding(&self, chunk_id: i64, embedding: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, bytes],
        )?;
        Ok(())
    }

    pub fn search_similar_chunks(&self, embedding: &[f32], top_k: u32) -> Result<Vec<(i64, f64)>> {
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let mut stmt = self.conn.prepare(
            "SELECT chunk_id, distance FROM vec_chunks WHERE embedding MATCH ?1 AND k = ?2"
        )?;
        let rows = stmt.query_map(params![bytes, top_k], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // --- FTS Operations ---

    pub fn search_fts(&self, query: &str, top_k: u32) -> Result<Vec<KeywordResult>> {
        // Search chunks FTS
        let mut stmt = self.conn.prepare(
            "SELECT c.file_path, snippet(fts_chunks, 0, '>>>', '<<<', '...', 32) as snip
             FROM fts_chunks fc
             JOIN chunks c ON fc.rowid = c.id
             WHERE fts_chunks MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;
        let chunk_results = stmt.query_map(params![query, top_k], |row| {
            Ok(KeywordResult {
                name: String::new(),
                entity_type: "CHUNK".to_string(),
                file_path: row.get(0)?,
                snippet: row.get(1)?,
            })
        })?;

        // Search entities FTS
        let mut stmt2 = self.conn.prepare(
            "SELECT e.name, e.type, e.file_path, e.qualified_name
             FROM fts_entities fe
             JOIN entities e ON fe.rowid = e.id
             WHERE fts_entities MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;
        let entity_results = stmt2.query_map(params![query, top_k], |row| {
            Ok(KeywordResult {
                name: row.get(0)?,
                entity_type: row.get(1)?,
                file_path: row.get(2)?,
                snippet: row.get(3)?,
            })
        })?;

        let mut results: Vec<KeywordResult> = chunk_results.filter_map(|r| r.ok()).collect();
        results.extend(entity_results.filter_map(|r| r.ok()));
        results.truncate(top_k as usize);
        Ok(results)
    }

    // --- Graph Neighbors (BFS) ---

    pub fn graph_neighbors(&self, entity_name: &str, depth: u32, direction: &str) -> Result<Vec<EntityRecord>> {
        // Find the entity ID first
        let entity_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM entities WHERE name = ?1 LIMIT 1",
            params![entity_name],
            |row| row.get(0),
        ).ok();

        let entity_id = match entity_id {
            Some(id) => id,
            None => return Ok(vec![]),
        };

        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![entity_id];
        visited.insert(entity_id);

        for _ in 0..depth {
            let mut next_frontier = vec![];
            for &eid in &frontier {
                let neighbor_ids = self.get_neighbor_ids(eid, direction)?;
                for nid in neighbor_ids {
                    if visited.insert(nid) {
                        next_frontier.push(nid);
                    }
                }
            }
            frontier = next_frontier;
        }

        // Collect all visited entity records (excluding root)
        visited.remove(&entity_id);
        let mut results = vec![];
        for eid in visited {
            if let Ok(rec) = self.get_entity_by_id(eid) {
                results.push(rec);
            }
        }
        Ok(results)
    }

    fn get_neighbor_ids(&self, entity_id: i64, direction: &str) -> Result<Vec<i64>> {
        let mut ids = vec![];
        if direction == "outgoing" || direction == "both" {
            let mut stmt = self.conn.prepare("SELECT target_id FROM relations WHERE source_id = ?1")?;
            let rows = stmt.query_map(params![entity_id], |row| row.get::<_, i64>(0))?;
            ids.extend(rows.filter_map(|r| r.ok()));
        }
        if direction == "incoming" || direction == "both" {
            let mut stmt = self.conn.prepare("SELECT source_id FROM relations WHERE target_id = ?1")?;
            let rows = stmt.query_map(params![entity_id], |row| row.get::<_, i64>(0))?;
            ids.extend(rows.filter_map(|r| r.ok()));
        }
        Ok(ids)
    }

    fn get_entity_by_id(&self, id: i64) -> Result<EntityRecord> {
        Ok(self.conn.query_row(
            "SELECT id, file_path, name, type, qualified_name, community_id FROM entities WHERE id = ?1",
            params![id],
            |row| Ok(EntityRecord {
                id: row.get(0)?,
                file_path: row.get(1)?,
                name: row.get(2)?,
                entity_type: row.get(3)?,
                qualified_name: row.get(4)?,
                community_id: row.get(5)?,
            }),
        )?)
    }

    pub fn get_chunk_by_id(&self, chunk_id: i64) -> Result<(String, String)> {
        Ok(self.conn.query_row(
            "SELECT text, file_path FROM chunks WHERE id = ?1",
            params![chunk_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    // --- File Hash (incremental indexing) ---

    pub fn get_file_hash(&self, file_path: &str) -> Result<Option<(i64, i64, String)>> {
        let result = self.conn.query_row(
            "SELECT mtime, size, hash FROM file_hashes WHERE file_path = ?1",
            params![file_path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn upsert_file_hash(&self, file_path: &str, mtime: i64, size: i64, hash: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_hashes (file_path, mtime, size, hash) VALUES (?1, ?2, ?3, ?4)",
            params![file_path, mtime, size, hash],
        )?;
        Ok(())
    }

    pub fn delete_file_data(&self, file_path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM chunks WHERE file_path = ?1", params![file_path])?;
        self.conn.execute("DELETE FROM entities WHERE file_path = ?1", params![file_path])?;
        self.conn.execute("DELETE FROM file_hashes WHERE file_path = ?1", params![file_path])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Database {
        Database::new(":memory:", false).expect("Failed to create in-memory DB")
    }

    #[test]
    fn test_entity_insertion_and_retrieval() {
        let db = setup_test_db();
        let id = db.insert_entity("test.rs", "TestEntity", "CLASS", "test::TestEntity");
        assert!(id.is_ok());

        let entity = db.get_entity("TestEntity", "test.rs").expect("Query failed");
        assert!(entity.is_some());
        let ed = entity.unwrap();
        assert_eq!(ed.entity.name, "TestEntity");
        assert_eq!(ed.entity.entity_type, "CLASS");
        assert_eq!(ed.entity.file_path, "test.rs");
    }

    #[test]
    fn test_relations() {
        let db = setup_test_db();
        let src_id = db.insert_entity("a.rs", "Src", "FUNC", "Src").unwrap();
        let tgt_id = db.insert_entity("b.rs", "Tgt", "FUNC", "Tgt").unwrap();
        
        let res = db.insert_relation(src_id, tgt_id, "CALLS", 1.0);
        assert!(res.is_ok());

        let entity = db.get_entity("Src", "a.rs").unwrap().unwrap();
        assert_eq!(entity.outgoing.len(), 1);
        assert_eq!(entity.outgoing[0].target_name, "Tgt");
        assert_eq!(entity.outgoing[0].relation_type, "CALLS");
    }

    #[test]
    fn test_chunk_insertion_and_deletion() {
        let db = setup_test_db();
        let chunk_id = db.insert_chunk("fn test() {}", "test.rs", Some(1), Some(5), None);
        assert!(chunk_id.is_ok());

        let chunk_ref = chunk_id.unwrap();
        let (text, path) = db.get_chunk_by_id(chunk_ref).unwrap();
        assert_eq!(text, "fn test() {}");
        assert_eq!(path, "test.rs");

        // Test cascading delete simulation (manual delete)
        db.delete_file_data("test.rs").unwrap();
        let missing = db.get_chunk_by_id(chunk_ref);
        assert!(missing.is_err());
    }
}
