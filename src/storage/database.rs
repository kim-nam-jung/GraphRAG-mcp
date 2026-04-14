use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use tracing::info;

use std::sync::Once;

static INIT_SQLITE_VEC: Once = Once::new();

fn init_sqlite_vec() {
    INIT_SQLITE_VEC.call_once(|| {
        unsafe {
            // SAFETY: sqlite3_vec_init is safely transmuted to automatic extension loader as required by sqlite-vec C API.
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

pub struct Database {
    conn: Connection,
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
            .with_context(|| format!("Failed to open database at {db_path}"))?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        if wal_mode {
            conn.execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA temp_store = MEMORY;
                PRAGMA mmap_size = 30000000000;
                PRAGMA page_size = 32768;
            ",
            )?;
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
                embedding float[384]
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS vec_entities USING vec0(
                entity_id INTEGER PRIMARY KEY,
                embedding float[384]
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

    // --- Transactions Operations ---

    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute_batch("BEGIN TRANSACTION;")?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute_batch("COMMIT;")?;
        Ok(())
    }

    // --- Entity Operations ---

    pub fn insert_entity(
        &self,
        file_path: &str,
        name: &str,
        entity_type: &str,
        qualified_name: &str,
    ) -> Result<i64> {
        let id = self.conn.query_row(
            "INSERT INTO entities (file_path, name, type, qualified_name) VALUES (?1, ?2, ?3, ?4) RETURNING id",
            params![file_path, name, entity_type, qualified_name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn update_entity_community(&self, name: &str, community_id: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE entities SET community_id = ?1 WHERE name = ?2",
            params![community_id, name],
        )?;
        Ok(())
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

        Ok(Some(EntityDetail {
            entity,
            incoming,
            outgoing,
        }))
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
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // --- Relation Operations ---

    pub fn insert_relation(
        &self,
        source_id: i64,
        target_id: i64,
        rel_type: &str,
        weight: f64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO relations (source_id, target_id, type, weight) VALUES (?1, ?2, ?3, ?4)",
            params![source_id, target_id, rel_type, weight],
        )?;
        Ok(())
    }

    pub fn insert_entity_vector(&self, entity_id: i64, embedding: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute(
            "INSERT INTO vec_entities (entity_id, embedding) VALUES (?1, ?2)",
            params![entity_id, bytes],
        )?;
        Ok(())
    }

    pub fn search_similar_entities(
        &self,
        query_embedding: &[f32],
        top_k: u32,
    ) -> Result<Vec<KeywordResult>> {
        let bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT e.name, e.type, e.file_path, e.qualified_name, v.distance
            FROM vec_entities v
            JOIN entities e ON v.entity_id = e.id
            WHERE v.embedding MATCH ?1 AND k = ?2
            ORDER BY v.distance ASC
            "#,
        )?;

        let rows = stmt.query_map(params![bytes, top_k], |row| {
            Ok(KeywordResult {
                name: row.get(0)?,
                entity_type: row.get(1)?,
                file_path: row.get(2)?,
                snippet: row.get(3)?, // Reusing snippet field for qualified_name logic
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(res) = r {
                results.push(res);
            }
        }
        Ok(results)
    }

    // --- Chunk Operations ---

    pub fn insert_chunk(
        &self,
        text: &str,
        file_path: &str,
        line_start: Option<i64>,
        line_end: Option<i64>,
        entity_id: Option<i64>,
    ) -> Result<i64> {
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
            "SELECT chunk_id, distance FROM vec_chunks WHERE embedding MATCH ?1 AND k = ?2",
        )?;
        let rows = stmt.query_map(params![bytes, top_k], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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
             LIMIT ?2",
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
             LIMIT ?2",
        )?;
        let entity_results = stmt2.query_map(params![query, top_k], |row| {
            Ok(KeywordResult {
                name: row.get(0)?,
                entity_type: row.get(1)?,
                file_path: row.get(2)?,
                snippet: row.get(3)?,
            })
        })?;

        let mut results: Vec<KeywordResult> =
            chunk_results.collect::<rusqlite::Result<Vec<_>>>()?;
        results.extend(entity_results.collect::<rusqlite::Result<Vec<_>>>()?);
        results.truncate(top_k as usize);
        Ok(results)
    }

    // --- Graph Neighbors (BFS) ---

    pub fn graph_neighbors(
        &self,
        entity_name: &str,
        depth: u32,
        direction: &str,
    ) -> Result<Vec<EntityRecord>> {
        // Find the entity ID first
        let entity_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM entities WHERE name = ?1 LIMIT 1",
                params![entity_name],
                |row| row.get(0),
            )
            .ok();

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
            let mut stmt = self
                .conn
                .prepare("SELECT target_id FROM relations WHERE source_id = ?1")?;
            let rows = stmt.query_map(params![entity_id], |row| row.get::<_, i64>(0))?;
            ids.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }
        if direction == "incoming" || direction == "both" {
            let mut stmt = self
                .conn
                .prepare("SELECT source_id FROM relations WHERE target_id = ?1")?;
            let rows = stmt.query_map(params![entity_id], |row| row.get::<_, i64>(0))?;
            ids.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
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

    pub fn upsert_file_hash(
        &self,
        file_path: &str,
        mtime: i64,
        size: i64,
        hash: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_hashes (file_path, mtime, size, hash) VALUES (?1, ?2, ?3, ?4)",
            params![file_path, mtime, size, hash],
        )?;
        Ok(())
    }

    pub fn delete_file_data(&self, file_path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE file_path = ?1)",
            params![file_path],
        )?;
        self.conn.execute(
            "DELETE FROM chunks WHERE file_path = ?1",
            params![file_path],
        )?;
        self.conn.execute(
            "DELETE FROM entities WHERE file_path = ?1",
            params![file_path],
        )?;
        self.conn.execute(
            "DELETE FROM file_hashes WHERE file_path = ?1",
            params![file_path],
        )?;
        Ok(())
    }

    pub fn get_all_relation_edges(&self) -> Result<Vec<(String, String, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e1.name, e2.name, r.weight 
             FROM relations r 
             JOIN entities e1 ON r.source_id = e1.id 
             JOIN entities e2 ON r.target_id = e2.id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn export_dashboard_html(&self, output_path: &str) -> Result<()> {
        use serde::Serialize;
        #[derive(Serialize)]
        struct EntityNode {
            id: String,
            name: String,
            group: i32,
            type_val: String,
            val: i32,
        }
        #[derive(Serialize)]
        struct LinkEdge {
            source: String,
            target: String,
            type_val: String,
        }
        #[derive(Serialize)]
        struct GraphData {
            nodes: Vec<EntityNode>,
            links: Vec<LinkEdge>,
        }

        let mut nodes = Vec::new();
        let mut links = Vec::new();

        let mut n_stmt = self.conn.prepare(
            "SELECT name || '@' || IFNULL(file_path, 'global') as id, max(name) as display_name, max(type), max(community_id) 
             FROM entities 
             WHERE name NOT LIKE '%test%' 
             GROUP BY name, file_path"
        )?;
        let entity_iter = n_stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let type_val: String = row.get(2)?;
            let community_id: Option<i32> = row.get(3)?;
            Ok(EntityNode {
                id: id.clone(),
                name,
                type_val,
                group: community_id.unwrap_or(1) as i32,
                val: 1,
            })
        })?;
        for e in entity_iter {
            if let Ok(n) = e { nodes.push(n); }
        }

        let mut r_stmt = self.conn.prepare(
            "SELECT e1.name || '@' || IFNULL(e1.file_path, 'global'), e2.name || '@' || IFNULL(e2.file_path, 'global'), r.type 
             FROM relations r
             JOIN entities e1 ON r.source_id = e1.id 
             JOIN entities e2 ON r.target_id = e2.id
             WHERE e1.name NOT LIKE '%test%' AND e2.name NOT LIKE '%test%'
             GROUP BY e1.name, e1.file_path, e2.name, e2.file_path, r.type"
        )?;
        let rel_iter = r_stmt.query_map([], |row| {
            let source: String = row.get(0)?;
            let target: String = row.get(1)?;
            let type_val: String = row.get(2)?;
            Ok(LinkEdge { source, target, type_val })
        })?;
        for r in rel_iter {
            if let Ok(l) = r { links.push(l); }
        }

        let data = GraphData { nodes, links };
        let json_data = serde_json::to_string(&data)?;

        let html_content = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>GraphRAG 3D Topology</title>
    <style>
        body {{ margin: 0; background: #0f172a; color: white; font-family: sans-serif; overflow: hidden; }}
        .title-card {{ position: absolute; top: 20px; left: 20px; background: rgba(255,255,255,0.1); padding: 15px; border-radius: 10px; z-index: 10; backdrop-filter: blur(5px); pointer-events: none; }}
        h1 {{ margin: 0 0 5px 0; font-size: 20px; }}
        p {{ margin: 0; font-size: 14px; opacity: 0.8; }}
        .node-info {{ background: rgba(0,0,0,0.8); padding: 5px 10px; border-radius: 4px;font-size: 12px; }}
        .type-badge {{ display: block; font-size: 10px; color: #a5b4fc; text-transform: uppercase; margin-bottom: 2px; }}
        .zoom-controls {{ position: absolute; bottom: 30px; right: 30px; display: flex; flex-direction: column; gap: 10px; z-index: 10; }}
        .zoom-btn {{ background: rgba(255,255,255,0.1); color: white; border: 1px solid rgba(255,255,255,0.2); border-radius: 8px; width: 44px; height: 44px; font-size: 24px; cursor: pointer; backdrop-filter: blur(5px); transition: all 0.2s; display: flex; align-items: center; justify-content: center; user-select: none; }}
        .zoom-btn:hover {{ background: rgba(255,255,255,0.25); transform: scale(1.05); }}
        .zoom-btn:active {{ transform: scale(0.95); }}
    </style>
    <script src="https://unpkg.com/force-graph@1.43.5/dist/force-graph.min.js"></script>
</head>
<body>
    <div class="title-card">
        <h1>GraphRAG Topology</h1>
        <p>Zero-Server 2D Dashboard</p>
    </div>
    
    <div class="zoom-controls">
        <div class="zoom-btn" onclick="Graph.zoom(Graph.zoom() * 1.5, 300)" title="Zoom In">+</div>
        <div class="zoom-btn" onclick="Graph.zoomToFit(400)" title="Reset View" style="font-size: 18px">⛶</div>
        <div class="zoom-btn" onclick="Graph.zoom(Graph.zoom() / 1.5, 300)" title="Zoom Out">−</div>
    </div>

    <div id="2d-graph"></div>

    <script>
        const GRAPH_DATA = {}; // Replaced by rust
        const groupColors = ['#ef4444', '#3b82f6', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'];
        
        GRAPH_DATA.links.forEach(link => {{
            const a = GRAPH_DATA.nodes.find(n => n.id === link.source);
            const b = GRAPH_DATA.nodes.find(n => n.id === link.target);
            if(a && b) {{
                if(!a.neighbors) a.neighbors = [];
                if(!b.neighbors) b.neighbors = [];
                a.neighbors.push(b);
                b.neighbors.push(a);
                if(!a.links) a.links = [];
                if(!b.links) b.links = [];
                a.links.push(link);
                b.links.push(link);
            }}
        }});

        let hoverNode = null;
        const highlightNodes = new Set();
        const highlightLinks = new Set();
        
        const Graph = ForceGraph()(document.getElementById('2d-graph'))
            .graphData(GRAPH_DATA)
            .nodeLabel(node => `<div class="node-info"><span class="type-badge">${{node.type_val}}</span>${{node.name}}</div>`)
            .nodeRelSize(3)
            .nodeCanvasObject((node, ctx, globalScale) => {{
                const isHovered = hoverNode === node;
                const isHighlight = highlightNodes.has(node);
                const isDimmed = hoverNode && !isHighlight;

                const nodeR = isHovered ? 4 : 3;
                ctx.beginPath();
                ctx.arc(node.x, node.y, nodeR, 0, 2 * Math.PI, false);
                ctx.fillStyle = groupColors[node.group % groupColors.length];
                if (isDimmed) ctx.globalAlpha = 0.1;
                ctx.fill();
                ctx.globalAlpha = 1;
                
                // Draw Text
                if (!isDimmed && (globalScale > 0.8 || isHighlight)) {{
                    const fontSize = (isHovered ? 14 : 12) / globalScale;
                    ctx.font = `${{fontSize}}px Sans-Serif`;
                    ctx.textAlign = 'center';
                    ctx.textBaseline = 'top';
                    ctx.fillStyle = isHovered ? '#fcd34d' : 'rgba(255, 255, 255, 0.9)';
                    let textY = node.y + nodeR + 2 / globalScale;
                    ctx.fillText(node.name, node.x, textY);
                }}
            }})
            .onNodeHover(node => {{
                highlightNodes.clear();
                highlightLinks.clear();
                if (node) {{
                    highlightNodes.add(node);
                    if (node.neighbors) node.neighbors.forEach(n => highlightNodes.add(n));
                    if (node.links) node.links.forEach(l => highlightLinks.add(l));
                }}
                hoverNode = node || null;
                document.getElementById('2d-graph').style.cursor = node ? 'pointer' : null;
            }})
            .linkLabel(link => `<div class="node-info" style="color:#fcd34d">${{link.type_val}}</div>`)
            .linkDirectionalArrowLength(6)
            .linkDirectionalArrowRelPos(1)
            .linkDirectionalParticles(link => link.type_val === 'contains' ? 0 : 2)
            .linkDirectionalParticleSpeed(0.005)
            .linkWidth(link => highlightLinks.has(link) ? 2 : 1)
            .linkColor(link => hoverNode ? (highlightLinks.has(link) ? '#fcd34d' : 'rgba(255,255,255,0.05)') : 'rgba(255,255,255,0.2)')
            .backgroundColor('#0f172a');
    </script>
</body>
</html>"#, json_data);

        std::fs::write(output_path, html_content)?;
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

        let entity = db
            .get_entity("TestEntity", "test.rs")
            .expect("Query failed");
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
