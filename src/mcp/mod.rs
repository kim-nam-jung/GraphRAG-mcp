use anyhow::Result;
use crate::config::Config;
use crate::search::SearchEngine;
use crate::storage::Database;
use crate::embedding::{HarrierModel, Tokenizer};
use crate::indexer::pipeline::IndexingPipeline;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Write};
use tokio::io::{self as tokio_io, AsyncBufReadExt};
use std::path::Path;
use tracing::{info, error};

pub struct McpServer<'a> {
    search: SearchEngine<'a>,
    db: &'a Database,
    harrier: &'a HarrierModel,
    tokenizer: &'a Tokenizer,
    cfg: &'a Config,
}

#[derive(Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

impl<'a> McpServer<'a> {
    pub fn new(
        search: SearchEngine<'a>,
        db: &'a Database,
        harrier: &'a HarrierModel,
        tokenizer: &'a Tokenizer,
        cfg: &'a Config,
    ) -> Self {
        Self { search, db, harrier, tokenizer, cfg }
    }

    pub async fn run_stdio(&self) -> Result<()> {
        let stdin = tokio_io::stdin();
        let mut reader = tokio_io::BufReader::new(stdin).lines();

        info!("MCP Server JSON-RPC loop started on stdio");

        while let Ok(Some(line)) = reader.next_line().await {
            match serde_json::from_str::<RpcRequest>(&line) {
                Ok(request) => {
                    if let Some(res) = self.handle_request(request) {
                        let mut out = io::stdout().lock();
                        writeln!(out, "{}", res)?;
                        out.flush()?;
                    }
                }
                Err(e) => {
                    error!("JSON Parse Error: {}", e);
                }
            }
        }
        Ok(())
    }

    fn handle_request(&self, req: RpcRequest) -> Option<String> {
        match req.method.as_str() {
            "initialize" => Some(self.handle_initialize(&req.id)),
            "tools/list" => Some(self.handle_tools_list(&req.id)),
            "tools/call" => Some(self.handle_tools_call(&req.id, req.params)),
            _ => None,
        }
    }

    fn handle_initialize(&self, id: &Option<Value>) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "GraphRAG-mcp",
                    "version": "1.0.0"
                }
            }
        }).to_string()
    }

    fn handle_tools_list(&self, id: &Option<Value>) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "keyword_search",
                        "description": "FTS5 keyword search over indexed code chunks and entities.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "top_k": { "type": "integer", "default": 10 }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "get_entity",
                        "description": "Get detailed information about a code entity including its relations.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "file": { "type": "string" }
                            },
                            "required": ["name", "file"]
                        }
                    },
                    {
                        "name": "global_search",
                        "description": "Broad search across all entities with relationship context.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "max_entities": { "type": "integer", "default": 50 }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "graph_neighbors",
                        "description": "BFS traversal to find neighboring entities in the knowledge graph.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "entity": { "type": "string" },
                                "depth": { "type": "integer", "default": 1 },
                                "direction": { "type": "string", "enum": ["incoming", "outgoing", "both"], "default": "outgoing" }
                            },
                            "required": ["entity"]
                        }
                    },
                    {
                        "name": "index_directory",
                        "description": "Index a directory's source code into the knowledge graph.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "local_search",
                        "description": "Semantic vector search combined with graph structural context.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "top_k": { "type": "integer", "default": 5 },
                                "graph_depth": { "type": "integer", "default": 1 }
                            },
                            "required": ["query"]
                        }
                    }
                ]
            }
        }).to_string()
    }

    fn handle_tools_call(&self, id: &Option<Value>, params: Option<Value>) -> String {
        let params = params.unwrap_or(json!({}));
        let tool_name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];

        let result = match tool_name {
            "keyword_search" => self.tool_keyword_search(args),
            "get_entity" => self.tool_get_entity(args),
            "global_search" => self.tool_global_search(args),
            "graph_neighbors" => self.tool_graph_neighbors(args),
            "index_directory" => self.tool_index_directory(args),
            "local_search" => self.tool_local_search(args),
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };

        match result {
            Ok(text) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }]
                }
            }).to_string(),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                }
            }).to_string(),
        }
    }

    fn tool_keyword_search(&self, args: &Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("");
        let top_k = args["top_k"].as_i64().unwrap_or(10) as u32;
        let results = self.db.search_fts(query, top_k)?;
        Ok(serde_json::to_string_pretty(&results)?)
    }

    fn tool_get_entity(&self, args: &Value) -> Result<String> {
        let name = args["name"].as_str().unwrap_or("");
        let file = args["file"].as_str().unwrap_or("");
        match self.db.get_entity(name, file)? {
            Some(detail) => Ok(serde_json::to_string_pretty(&detail)?),
            None => Ok(json!({"error": "Entity not found"}).to_string()),
        }
    }

    fn tool_global_search(&self, args: &Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("");
        let max_entities = args["max_entities"].as_i64().unwrap_or(50) as u32;
        self.search.global_search(query, max_entities)
    }

    fn tool_graph_neighbors(&self, args: &Value) -> Result<String> {
        let entity = args["entity"].as_str().unwrap_or("");
        let depth = args["depth"].as_i64().unwrap_or(1) as u32;
        let direction = args["direction"].as_str().unwrap_or("outgoing");
        let neighbors = self.db.graph_neighbors(entity, depth, direction)?;
        Ok(serde_json::to_string_pretty(&neighbors)?)
    }

    fn tool_index_directory(&self, args: &Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or("");
        let pipeline = IndexingPipeline::new(self.db, self.harrier, self.tokenizer, self.cfg);
        pipeline.run_indexing(Path::new(path))?;
        Ok(json!({"status": "ok", "indexed_path": path}).to_string())
    }

    fn tool_local_search(&self, args: &Value) -> Result<String> {
        let query = args["query"].as_str().unwrap_or("");
        let top_k = args["top_k"].as_i64().unwrap_or(5) as u32;
        let graph_depth = args["graph_depth"].as_i64().unwrap_or(1) as u32;
        self.search.local_search(query, top_k, graph_depth)
    }
}
