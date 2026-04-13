package graph

import (
	"fmt"
	"log"
	"math/rand"

	"graphrag-mcp/internal/storage"
)

// LeidenNative represents a pure-Go implementation of community detection.
// It applies a Louvain-style local modularity move phase, mirroring the foundation of Leiden.
type LeidenNative struct {
	Resolution float64
}

func NewLeidenNative(resolution float64) *LeidenNative {
	if resolution <= 0 {
		resolution = 1.0
	}
	return &LeidenNative{
		Resolution: resolution,
	}
}

// Calculate applies native graph partitioning and updates the database
func (l *LeidenNative) Calculate(db *storage.Database) error {
	log.Println("[Leiden-Native] Starting pure-Go Community Detection (Local Move)...")

	// 1. Fetch nodes (Initialize each as its own community)
	nodes := make(map[int]int)
	rows, err := db.DB.Query("SELECT id FROM entities")
	if err == nil {
		for rows.Next() {
			var id int
			if rows.Scan(&id) == nil {
				nodes[id] = id
			}
		}
		rows.Close()
	}

	if len(nodes) == 0 {
		return nil // No entities to process
	}

	// 2. Fetch edges and build adjacency list
	type Edge struct {
		target int
		weight float64
	}
	adj := make(map[int][]Edge)
	
	edgeRows, err := db.DB.Query("SELECT source_id, target_id FROM relations")
	if err == nil {
		for edgeRows.Next() {
			var s, t int
			if edgeRows.Scan(&s, &t) == nil {
				// Undirected links for standard modularity optimization
				adj[s] = append(adj[s], Edge{target: t, weight: 1.0})
				adj[t] = append(adj[t], Edge{target: s, weight: 1.0})
			}
		}
		edgeRows.Close()
	}

	// 3. Modularity Maximization Phase (Iterative Local Move)
	nodeList := make([]int, 0, len(nodes))
	for id := range nodes {
		nodeList = append(nodeList, id)
	}

	// Prepare arrays for modularity calculation
	nodeDegree := make(map[int]float64)
	var totalWeight float64 = 0.0

	for s, edges := range adj {
		for _, e := range edges {
			nodeDegree[s] += e.weight
			totalWeight += e.weight
		}
	}
	m2 := totalWeight // totalWeight is actually 2m because edges are added symmetrically (undirected)

	changed := true
	iters := 0
	maxIters := 15

	for changed && iters < maxIters {
		changed = false
		iters++
		rand.Shuffle(len(nodeList), func(i, j int) { nodeList[i], nodeList[j] = nodeList[j], nodeList[i] })

		// Compute Sigma_tot for each community
		commSigmaTot := make(map[int]float64)
		for node, comm := range nodes {
			commSigmaTot[comm] += nodeDegree[node]
		}

		for _, nodeID := range nodeList {
			currentComm := nodes[nodeID]
			ki := nodeDegree[nodeID]

			// Aggregate edge weights to neighboring communities
			commWeights := make(map[int]float64)
			for _, edge := range adj[nodeID] {
				neighborComm := nodes[edge.target]
				commWeights[neighborComm] += edge.weight
			}

			// Remove node from its current community for the calculation
			commSigmaTot[currentComm] -= ki

			bestComm := currentComm
			bestGain := 0.0

			for comm, k_i_in := range commWeights {
				if comm == currentComm {
					continue
				}
				
				// Modularity delta Q = k_i_in - resolution * (Sigma_tot * k_i) / m2
				var expected float64 = 0
				if m2 > 0 {
					expected = l.Resolution * (commSigmaTot[comm] * ki) / m2
				}
				deltaQ := k_i_in - expected

				if deltaQ > bestGain {
					bestGain = deltaQ
					bestComm = comm
				}
			}

			// Execute local move
			if bestComm != currentComm {
				nodes[nodeID] = bestComm
				commSigmaTot[bestComm] += ki
				changed = true
			} else {
				commSigmaTot[currentComm] += ki
			}
		}
	}

	log.Printf("[Leiden-Native] Converged after %d iterations.", iters)

	// 4. Update the Database with computed macro-communities
	tx, err := db.DB.Begin()
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}

	updateStmt, _ := tx.Prepare("UPDATE entities SET community_id = ? WHERE id = ?")
	insertCommStmt, _ := tx.Prepare("INSERT OR IGNORE INTO communities (id, level) VALUES (?, 0)")
	
	uniqueComms := make(map[int]bool)

	for nodeID, commID := range nodes {
		if !uniqueComms[commID] {
			insertCommStmt.Exec(commID)
			uniqueComms[commID] = true
		}
		updateStmt.Exec(commID, nodeID)
	}

	updateStmt.Close()
	insertCommStmt.Close()
	
	if err = tx.Commit(); err != nil {
		return fmt.Errorf("failed to commit community assignments: %w", err)
	}

	log.Printf("[Leiden-Native] Successfully partitioned %d nodes into %d communities.", len(nodes), len(uniqueComms))
	return nil
}
