use anyhow::Result;
use axum::{routing::get, Json, Router};
use rusqlite::Connection;
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tracing::{error, info};

#[derive(Serialize)]
struct Entity {
    name: String,
    entity_type: String,
    qualified_name: String,
    community: i32,
}

#[derive(Serialize)]
struct Relation {
    source: String,
    target: String,
    relation_type: String,
}

#[derive(Serialize)]
struct GraphData {
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

fn fetch_graph_data() -> Result<GraphData> {
    let conn = Connection::open("./data/graph.db")?;
    
    let count: i64 = conn.query_row("SELECT count(*) FROM entities", [], |row| row.get(0)).unwrap_or(-1);
    error!("Entities count in web.rs: {}", count);

    let mut stmt = conn.prepare("SELECT name, type, qualified_name, community_id FROM entities")?;
    let entity_iter = stmt.query_map([], |row| {
        Ok(Entity {
            name: row.get(0)?,
            entity_type: row.get(1)?,
            qualified_name: row.get(2)?,
            community: row.get(3).unwrap_or(1),
        })
    })?;

    let mut entities = Vec::new();
    for e in entity_iter {
        match e {
            Ok(entity) => entities.push(entity),
            Err(err) => error!("Entity row error: {}", err),
        }
    }

    let mut stmt = conn.prepare(
        "SELECT e1.name as source, e2.name as target, r.type as relation_type 
         FROM relations r
         JOIN entities e1 ON r.source_id = e1.id
         JOIN entities e2 ON r.target_id = e2.id"
    )?;
    let rel_iter = stmt.query_map([], |row| {
        Ok(Relation {
            source: row.get(0)?,
            target: row.get(1)?,
            relation_type: row.get(2)?,
        })
    })?;

    let mut relations = Vec::new();
    for r in rel_iter {
        match r {
            Ok(rel) => relations.push(rel),
            Err(err) => error!("Relation row error: {}", err),
        }
    }

    Ok(GraphData {
        entities,
        relations,
    })
}

async fn api_graph() -> Result<Json<GraphData>, String> {
    match fetch_graph_data() {
        Ok(data) => Ok(Json(data)),
        Err(e) => {
            error!("Failed to fetch graph data: {}", e);
            Err(e.to_string())
        }
    }
}

async fn api_debug() -> String {
    use std::env;
    let cwd = env::current_dir().unwrap_or_default();
    let conn = match Connection::open("./data/graph.db") {
        Ok(c) => c,
        Err(e) => return format!("Failed to open DB: {}", e),
    };
    let count: i64 = conn.query_row("SELECT count(*) FROM entities", [], |row| row.get(0)).unwrap_or(-1);
    format!("CWD: {:?}\nEntities Count: {}", cwd, count)
}

pub async fn start_server() {
    let serve_dir = ServeDir::new("./dashboard/dist");

    let app = Router::new()
        .route("/api/graph", get(api_graph))
        .route("/api/debug", get(api_debug))
        .fallback_service(serve_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Starting Web Dashboard on http://{}", addr);

    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Axum server error: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to bind Axum server to {}: {}", addr, e);
        }
    }
}
