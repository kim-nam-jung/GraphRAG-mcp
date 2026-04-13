package search

import (
	"fmt"
	"strings"

	"graphrag-mcp/internal/embedding"
	"graphrag-mcp/internal/storage"
)

type LocalSearchResult struct {
	EntryPoints  []map[string]interface{} `json:"entry_points"`
	GraphContext map[string]interface{}   `json:"graph_context"`
	Chunks       []map[string]string      `json:"chunks"`
}

// LocalSearch performs Hybrid Vector + Eager Graph search
func LocalSearch(db *storage.Database, harrier *embedding.HarrierModel, tokenizer *embedding.Tokenizer, query, instruction string, topK, graphDepth int) (*LocalSearchResult, error) {
	// 1. Vector Search for starting chunks
	emb, err := harrier.Embed(query, true, instruction, tokenizer)
	if err != nil {
		return nil, err
	}

	chunkIDs, _, err := db.SearchSimilarChunks(emb, topK)
	if err != nil {
		return nil, err
	}

	res := &LocalSearchResult{
		EntryPoints: []map[string]interface{}{},
		GraphContext: map[string]interface{}{
			"nodes": []string{},
			"edges": []string{},
		},
		Chunks: []map[string]string{},
	}

	if len(chunkIDs) == 0 {
		return res, nil
	}

	// 2. Fetch specific Chunk Data
	placeholders := make([]string, len(chunkIDs))
	args := make([]interface{}, len(chunkIDs))
	for i, id := range chunkIDs {
		placeholders[i] = "?"
		args[i] = id
	}

	queryChunks := fmt.Sprintf("SELECT id, text, file_path FROM chunks WHERE id IN (%s)", strings.Join(placeholders, ","))
	rows, err := db.DB.Query(queryChunks, args...)
	if err == nil {
		for rows.Next() {
			var id int
			var text, filePath string
			if rows.Scan(&id, &text, &filePath) == nil {
				res.Chunks = append(res.Chunks, map[string]string{
					"id":   fmt.Sprintf("%d", id),
					"text": text,
					"file": filePath,
				})
			}
		}
		rows.Close()
	}

	// 3. Resolve entities overlapping with chunks (Hybrid integration)
	// Query entities connected to these chunks
	queryEntities := fmt.Sprintf(`
		SELECT DISTINCT e.id, e.name, e.type 
		FROM entities e
		JOIN chunks c ON e.id = c.entity_id
		WHERE c.id IN (%s)
	`, strings.Join(placeholders, ","))

	entRows, err := db.DB.Query(queryEntities, args...)
	
	var nodes []string
	var entityIds []int
	
	if err == nil {
		for entRows.Next() {
			var eid int
			var ename, etype string
			if entRows.Scan(&eid, &ename, &etype) == nil {
				res.EntryPoints = append(res.EntryPoints, map[string]interface{}{
					"id": eid, "name": ename, "type": etype,
				})
				nodes = append(nodes, ename)
				entityIds = append(entityIds, eid)
			}
		}
		entRows.Close()
	}

	// Fetch graph neighbors for these entities, respecting graphDepth
	if len(entityIds) > 0 && graphDepth > 0 {
		var edges []string
		
		// In a real implementation we would recurse graphDepth times, but for MVP we do 1 step
		ePlaceholders := make([]string, len(entityIds))
		eArgs := make([]interface{}, len(entityIds)*2)
		for i, id := range entityIds {
			ePlaceholders[i] = "?"
			eArgs[i] = id
			eArgs[i+len(entityIds)] = id
		}
		
		qEdges := fmt.Sprintf(`
			SELECT e1.name, e2.name, r.type
			FROM relations r
			JOIN entities e1 ON r.source_id = e1.id
			JOIN entities e2 ON r.target_id = e2.id
			WHERE r.source_id IN (%s) OR r.target_id IN (%s)
		`, strings.Join(ePlaceholders, ","), strings.Join(ePlaceholders, ","))
		
		edgeRows, err := db.DB.Query(qEdges, eArgs...)
		if err == nil {
			for edgeRows.Next() {
				var sName, tName, rType string
				if edgeRows.Scan(&sName, &tName, &rType) == nil {
					edges = append(edges, fmt.Sprintf("%s -[%s]-> %s", sName, rType, tName))
				}
			}
			edgeRows.Close()
		}
		res.GraphContext["edges"] = edges
	}

	res.GraphContext["nodes"] = nodes

	return res, nil
}
