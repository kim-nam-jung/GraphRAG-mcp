package search

import (
	"testing"
	"graphrag-mcp/internal/storage"
)

func TestGraphNeighbors_BFS(t *testing.T) {
	db, err := storage.InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("failed to init db: %v", err)
	}
	defer db.Close()

	// A -> B -> C
	// A <- D
	queries := []string{
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (1, 'A', 't', 'q1', 'f1')",
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (2, 'B', 't', 'q2', 'f2')",
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (3, 'C', 't', 'q3', 'f3')",
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (4, 'D', 't', 'q4', 'f4')",
		"INSERT INTO relations (source_id, target_id, type) VALUES (1, 2, 'CALLS')",
		"INSERT INTO relations (source_id, target_id, type) VALUES (2, 3, 'CALLS')",
		"INSERT INTO relations (source_id, target_id, type) VALUES (4, 1, 'CALLS')",
	}
	for _, q := range queries {
		db.DB.Exec(q)
	}

	// Depth 1, Both
	res, err := GraphNeighbors(db, "A", 1, "both")
	if err != nil {
		t.Fatalf("GraphNeighbors failed: %v", err)
	}

	m := res.(map[string]interface{})
	nodes := m["nodes"].(map[int]string)

	if len(nodes) != 3 { // A, B, D
		t.Errorf("expected 3 nodes in depth 1 (A, B, D), got %d", len(nodes))
	}
	if nodes[1] != "A" || nodes[2] != "B" || nodes[4] != "D" {
		t.Errorf("nodes content mismatch")
	}

	// Depth 2, Outgoing
	res2, _ := GraphNeighbors(db, "A", 2, "outgoing")
	m2 := res2.(map[string]interface{})
	nodes2 := m2["nodes"].(map[int]string)

	if len(nodes2) != 3 { // A, B, C
		t.Errorf("expected 3 nodes in depth 2 outgoing (A, B, C), got %d", len(nodes2))
	}
	if _, ok := nodes2[4]; ok {
		t.Errorf("expected D to be excluded in outgoing direction")
	}
}
