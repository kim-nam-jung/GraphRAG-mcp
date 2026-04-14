use anyhow::Result;
use gryf::{Graph, core::marker::Undirected, core::id::{VertexId, IdType}};
use leiden_rs::{Leiden, LeidenConfig};
use std::collections::HashMap;
use tracing::info;

pub struct LeidenNative {
    pub resolution: f32,
    pub graph: Graph<String, f64, Undirected>,
    node_map: HashMap<String, VertexId>,
    reverse_map: HashMap<usize, String>,
}

impl LeidenNative {
    pub fn new(resolution: f32) -> Self {
        Self {
            resolution,
            graph: Graph::new_undirected(),
            node_map: HashMap::new(),
            reverse_map: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, source: &str, target: &str, weight: f64) {
        let src_idx = *self.node_map.entry(source.to_string()).or_insert_with(|| {
            let idx = self.graph.add_vertex(source.to_string());
            // Store string lookup by usize using as_usize()
            self.reverse_map.insert(idx.as_usize(), source.to_string());
            idx
        });
        
        let tgt_idx = *self.node_map.entry(target.to_string()).or_insert_with(|| {
            let idx = self.graph.add_vertex(target.to_string());
            self.reverse_map.insert(idx.as_usize(), target.to_string());
            idx
        });

        self.graph.add_edge(src_idx, tgt_idx, weight);
    }

    /// Run the Leiden algorithm (Local Move -> Refinement -> Aggregation)
    pub fn calculate(&mut self) -> Result<HashMap<String, u32>> {
        info!("Running Leiden community detection clustering on {} nodes...", self.graph.vertex_count());
        
        if self.graph.vertex_count() == 0 {
            return Ok(HashMap::new());
        }

        let mut config = LeidenConfig::default();
        config.resolution = self.resolution as f64;
        let leiden = Leiden::new(config);
        
        let result = leiden.run(&self.graph).map_err(|e| anyhow::anyhow!("Leiden execution failed: {:?}", e))?;
        
        let mut communities = HashMap::new();
        for (node_idx, comm_id) in result.partition.iter() {
            if let Some(name) = self.reverse_map.get(&node_idx) {
                communities.insert(name.clone(), comm_id as u32);
            }
        }
        
        info!("Identified {} basic communities. Quality: {:.4}", result.partition.num_communities(), result.quality);
        Ok(communities)
    }
}
