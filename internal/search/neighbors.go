package search

import (
	"fmt"
	"strings"

	"graphrag-mcp/internal/storage"
)

// GraphNeighbors retrieves depth-N neighbors for a given entity.
// It corresponds to the 'graph_neighbors' MCP tool.
func GraphNeighbors(db *storage.Database, entityName string, depth int, direction string) (interface{}, error) {
	// MVP implementation: BFS up to depth on the relations table.

	// 1. Resolve root entity
	var rootID int
	err := db.DB.QueryRow("SELECT id FROM entities WHERE name = ? LIMIT 1", entityName).Scan(&rootID)
	if err != nil {
		return nil, fmt.Errorf("root entity not found: %w", err)
	}

	nodes := map[int]string{rootID: entityName}
	edges := []map[string]interface{}{}

	currentQueue := []int{rootID}
	
	for d := 0; d < depth; d++ {
		var nextQueue []int
		
		for _, nodeID := range currentQueue {
			// Outgoing edges
			if direction == "outgoing" || direction == "both" {
				qOut := `SELECT target_id, type FROM relations WHERE source_id = ?`
				rows, err := db.DB.Query(qOut, nodeID)
				if err == nil {
					for rows.Next() {
						var tID int
						var typ string
						if err := rows.Scan(&tID, &typ); err == nil {
							edges = append(edges, map[string]interface{}{
								"source": nodeID, "target": tID, "type": typ,
							})
							if _, ok := nodes[tID]; !ok {
								nodes[tID] = ""
								nextQueue = append(nextQueue, tID)
							}
						}
					}
					rows.Close()
				}
			}
			
			// Incoming edges
			if direction == "incoming" || direction == "both" {
				qIn := `SELECT source_id, type FROM relations WHERE target_id = ?`
				rows, err := db.DB.Query(qIn, nodeID)
				if err == nil {
					for rows.Next() {
						var sID int
						var typ string
						if err := rows.Scan(&sID, &typ); err == nil {
							edges = append(edges, map[string]interface{}{
								"source": sID, "target": nodeID, "type": typ,
							})
							if _, ok := nodes[sID]; !ok {
								nodes[sID] = ""
								nextQueue = append(nextQueue, sID)
							}
						}
					}
					rows.Close()
				}
			}
		}
		currentQueue = nextQueue
	}

	// Resolve target names
	if len(nodes) > 1 {
		var ids []string
		for id := range nodes {
			ids = append(ids, fmt.Sprintf("%d", id))
		}
		resolveQ := fmt.Sprintf("SELECT id, name FROM entities WHERE id IN (%s)", strings.Join(ids, ","))
		rows, err := db.DB.Query(resolveQ)
		if err == nil {
			for rows.Next() {
				var id int
				var name string
				if err := rows.Scan(&id, &name); err == nil {
					nodes[id] = name
				}
			}
			rows.Close()
		}
	}

	return map[string]interface{}{
		"root":  entityName,
		"nodes": nodes,
		"edges": edges,
	}, nil
}
