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

    let mut stmt = conn.prepare("SELECT name, type, qualified_name, community FROM entities")?;
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
        if let Ok(entity) = e {
            entities.push(entity);
        }
    }

    let mut stmt = conn.prepare("SELECT source, target, relation_type FROM relations")?;
    let rel_iter = stmt.query_map([], |row| {
        Ok(Relation {
            source: row.get(0)?,
            target: row.get(1)?,
            relation_type: row.get(2)?,
        })
    })?;

    let mut relations = Vec::new();
    for r in rel_iter {
        if let Ok(rel) = r {
            relations.push(rel);
        }
    }

    Ok(GraphData {
        entities,
        relations,
    })
}

async fn api_graph() -> Json<GraphData> {
    match fetch_graph_data() {
        Ok(data) => Json(data),
        Err(e) => {
            error!("Failed to fetch graph data: {}", e);
            Json(GraphData {
                entities: vec![],
                relations: vec![],
            })
        }
    }
}

pub async fn start_server() {
    let serve_dir = ServeDir::new("./dashboard/dist");

    let app = Router::new()
        .route("/api/graph", get(api_graph))
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
