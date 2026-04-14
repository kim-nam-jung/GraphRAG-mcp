import { useEffect, useState, useMemo, useRef } from 'react';
import ForceGraph3D from 'react-force-graph-3d';
import type { ForceGraphMethods } from 'react-force-graph-3d';
import { Network, Activity } from 'lucide-react';


interface Entity {
  name: string;
  entity_type: string;
  qualified_name: string;
  community: number;
}

interface Relation {
  source: string;
  target: string;
  relation_type: string;
}

interface GraphData {
  entities: Entity[];
  relations: Relation[];
}

interface Node {
  id: string;
  name: string;
  group: number;
  type: string;
  val: number;
}

interface Link {
  source: string;
  target: string;
  type: string;
}

interface Node3D extends Node {
  x?: number;
  y?: number;
  z?: number;
}

function App() {
  const [data, setData] = useState<{ nodes: Node[], links: Link[] }>({ nodes: [], links: [] });
  const [mode, setMode] = useState<'force' | 'dag'>('force');
  const fgRef = useRef<ForceGraphMethods | undefined>(undefined);

  useEffect(() => {
    // In production, this fetches from the Rust Axum server
    fetch('/api/graph')
      .then(res => res.json())
      .then((payload: GraphData) => {
        const nodes = payload.entities.map(e => ({
          id: e.qualified_name,
          name: e.name,
          type: e.entity_type,
          group: e.community || 1,
          val: 1
        }));
        const links = payload.relations.map(r => ({
          source: r.source,
          target: r.target,
          type: r.relation_type
        }));
        setData({ nodes, links });
      })
      .catch((err: unknown) => {
        console.error("Failed to fetch graph data", err);
        // Fallback demo data for dev testing
        setData({
          nodes: [
            { id: "A", name: "main", type: "FUNCTION", group: 1, val: 2 },
            { id: "B", name: "utils", type: "CLASS", group: 2, val: 1 },
            { id: "C", name: "parser", type: "CLASS", group: 2, val: 1 }
          ],
          links: [
            { source: "A", target: "B", type: "CALLS" },
            { source: "A", target: "C", type: "CALLS" },
          ]
        });
      });
  }, []);

  const handleNodeClick = (nodeObj: object) => {
    const node = nodeObj as Node3D;
    if (fgRef.current && node.x !== undefined && node.y !== undefined && node.z !== undefined) {
      const distance = 40;
      const distRatio = 1 + distance/Math.hypot(node.x, node.y, node.z);
      fgRef.current.cameraPosition(
        { x: node.x * distRatio, y: node.y * distRatio, z: node.z * distRatio },
        { x: node.x, y: node.y, z: node.z },
        3000
      );
    }
  };

  const groupColors = useMemo(() => [
    '#ef4444', '#3b82f6', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'
  ], []);

  return (
    <>
      <div className="title-card glass-panel">
        <h1>GraphRAG Topology</h1>
        <p>Interactive 3D Knowledge Graph</p>
      </div>

      <div className="controls glass-panel">
        <button 
          className={`btn-toggle ${mode === 'force' ? 'active' : ''}`}
          onClick={() => { setMode('force'); }}
        >
          <Network size={18} />
          Force Physics
        </button>
        <button 
          className={`btn-toggle ${mode === 'dag' ? 'active' : ''}`}
          onClick={() => { setMode('dag'); }}
        >
          <Activity size={18} />
          DAG Hierarchy
        </button>
      </div>

      <ForceGraph3D
        ref={fgRef}
        graphData={data}
        nodeLabel={(nodeObj: object) => {
          const n = nodeObj as Node3D;
          return `<div class="node-tooltip"><div class="type">${n.type}</div>${n.name}</div>`;
        }}
        nodeColor={(nodeObj: object) => {
          const n = nodeObj as Node3D;
          return groupColors[n.group % groupColors.length];
        }}
        nodeRelSize={6}
        linkDirectionalArrowLength={3.5}
        linkDirectionalArrowRelPos={1}
        linkWidth={0.5}
        linkOpacity={0.3}
        linkColor={() => 'rgba(255,255,255,0.2)'}
        dagMode={mode === 'dag' ? 'td' : undefined}
        dagLevelDistance={mode === 'dag' ? 50 : undefined}
        onNodeClick={handleNodeClick}
        backgroundColor="#0f172a"
      />
    </>
  );
}

export default App;
