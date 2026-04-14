import sqlite3, json
c = sqlite3.connect('/home/skawn1057/Development/GraphRAG-mcp/data/graph.db')

nodes = []
for r in c.execute('SELECT name, type, community_id FROM entities'):
    nodes.append({'id': r[0], 'name': r[0], 'type_val': r[1], 'group': r[2] if r[2] else 1, 'val': 1})

links = []
for r in c.execute('SELECT e1.name, e2.name, r.type FROM relations r JOIN entities e1 ON r.source_id = e1.id JOIN entities e2 ON r.target_id = e2.id'):
    links.append({'source': r[0], 'target': r[1], 'type_val': r[2]})

data = {'nodes': nodes, 'links': links}

html = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>GraphRAG 2D Topology</title>
    <style>
        body { margin: 0; background: #0f172a; color: white; font-family: sans-serif; overflow: hidden; }
        .title-card { position: absolute; top: 20px; left: 20px; background: rgba(255,255,255,0.1); padding: 15px; border-radius: 10px; z-index: 10; backdrop-filter: blur(5px); pointer-events: none; }
        h1 { margin: 0 0 5px 0; font-size: 20px; }
        p { margin: 0; font-size: 14px; opacity: 0.8; }
        .node-info { background: rgba(0,0,0,0.8); padding: 5px 10px; border-radius: 4px;font-size: 12px; }
        .type-badge { display: block; font-size: 10px; color: #a5b4fc; text-transform: uppercase; margin-bottom: 2px; }
        .zoom-controls { position: absolute; bottom: 30px; right: 30px; display: flex; flex-direction: column; gap: 10px; z-index: 10; }
        .zoom-btn { background: rgba(255,255,255,0.1); color: white; border: 1px solid rgba(255,255,255,0.2); border-radius: 8px; width: 44px; height: 44px; font-size: 24px; cursor: pointer; backdrop-filter: blur(5px); transition: all 0.2s; display: flex; align-items: center; justify-content: center; user-select: none; }
        .zoom-btn:hover { background: rgba(255,255,255,0.25); transform: scale(1.05); }
        .zoom-btn:active { transform: scale(0.95); }
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
        const GRAPH_DATA = """ + json.dumps(data) + """;
        const groupColors = ['#ef4444', '#3b82f6', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'];
        
        GRAPH_DATA.links.forEach(link => {
            const a = GRAPH_DATA.nodes.find(n => n.id === link.source);
            const b = GRAPH_DATA.nodes.find(n => n.id === link.target);
            if(a && b) {
                if(!a.neighbors) a.neighbors = [];
                if(!b.neighbors) b.neighbors = [];
                a.neighbors.push(b);
                b.neighbors.push(a);
                if(!a.links) a.links = [];
                if(!b.links) b.links = [];
                a.links.push(link);
                b.links.push(link);
            }
        });

        let hoverNode = null;
        const highlightNodes = new Set();
        const highlightLinks = new Set();
        
        const Graph = ForceGraph()(document.getElementById('2d-graph'))
            .graphData(GRAPH_DATA)
            .nodeLabel(node => '<div class="node-info"><span class="type-badge">' + node.type_val + '</span>' + node.name + '</div>')
            .nodeRelSize(3)
            .nodeCanvasObject((node, ctx, globalScale) => {
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
                
                if (!isDimmed && (globalScale > 0.8 || isHighlight)) {
                    const fontSize = (isHovered ? 14 : 12) / globalScale;
                    ctx.font = `${fontSize}px Sans-Serif`;
                    ctx.textAlign = 'center';
                    ctx.textBaseline = 'top';
                    ctx.fillStyle = isHovered ? '#fcd34d' : 'rgba(255, 255, 255, 0.9)';
                    let textY = node.y + nodeR + 2 / globalScale;
                    ctx.fillText(node.name, node.x, textY);
                }
            })
            .onNodeHover(node => {
                highlightNodes.clear();
                highlightLinks.clear();
                if (node) {
                    highlightNodes.add(node);
                    if (node.neighbors) node.neighbors.forEach(n => highlightNodes.add(n));
                    if (node.links) node.links.forEach(l => highlightLinks.add(l));
                }
                hoverNode = node || null;
                document.getElementById('2d-graph').style.cursor = node ? 'pointer' : null;
            })
            .linkLabel(link => '<div class="node-info" style="color:#fcd34d">' + link.type_val + '</div>')
            .linkDirectionalArrowLength(6)
            .linkDirectionalArrowRelPos(1)
            .linkDirectionalParticles(link => link.type_val === 'contains' ? 0 : 2)
            .linkDirectionalParticleSpeed(0.005)
            .linkWidth(link => highlightLinks.has(link) ? 2 : 1)
            .linkColor(link => hoverNode ? (highlightLinks.has(link) ? '#fcd34d' : 'rgba(255,255,255,0.05)') : 'rgba(255,255,255,0.2)')
            .backgroundColor('#0f172a');
    </script>
</body></html>"""

with open('/home/skawn1057/Development/GraphRAG-mcp/data/dashboard.html', 'w', encoding='utf-8') as f:
    f.write(html)
