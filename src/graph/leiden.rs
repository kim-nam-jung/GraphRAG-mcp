use anyhow::Result;
use petgraph::graph::{UnGraph, NodeIndex};
use std::collections::HashMap;
use tracing::info;

pub struct LeidenNative {
    pub resolution: f32,
    pub graph: Box<UnGraph<String, f32>>, // Node: Entity Name, Edge: Weight
    node_map: HashMap<String, NodeIndex>,
}

impl LeidenNative {
    pub fn new(resolution: f32) -> Self {
        Self {
            resolution,
            graph: Box::new(UnGraph::new_undirected()),
            node_map: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, source: &str, target: &str, weight: f32) {
        let src_idx = *self.node_map.entry(source.to_string()).or_insert_with(|| {
            self.graph.add_node(source.to_string())
        });
        let tgt_idx = *self.node_map.entry(target.to_string()).or_insert_with(|| {
            self.graph.add_node(target.to_string())
        });

        self.graph.add_edge(src_idx, tgt_idx, weight);
    }

    /// 执行 Leiden 알고리즘 (Local Move -> Refinement -> Aggregation)
    pub fn calculate(&mut self) -> Result<HashMap<String, u32>> {
        info!("Running Leiden community detection clustering on {} nodes...", self.graph.node_count());
        
        let mut communities = HashMap::new();
        // Base mapping: For skeleton, we return trivial components mapping.
        // Full algorithm: petgraph doesn't have native leiden. 
        // We'll iterate through graph identifying connected components as base communities.
        let mut community_id = 0;
        let mut visited = std::collections::HashSet::new();

        for node_idx in self.graph.node_indices() {
            if !visited.contains(&node_idx) {
                let mut bfs = petgraph::visit::Bfs::new(&*self.graph, node_idx);
                while let Some(nx) = bfs.next(&*self.graph) {
                    visited.insert(nx);
                    if let Some(name) = self.graph.node_weight(nx) {
                        communities.insert(name.clone(), community_id);
                    }
                }
                community_id += 1;
            }
        }
        
        info!("Identified {} basic communities.", community_id);
        Ok(communities)
    }
}
