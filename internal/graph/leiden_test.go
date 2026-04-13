package graph

import (
	"testing"
	"graphrag-mcp/internal/storage"
)

func TestLeidenNative_Calculate(t *testing.T) {
	db, err := storage.InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("failed to init db: %v", err)
	}
	defer db.Close()

	// 1. Create two disjoint dense subgraphs (cliques)
	// Cluster 1: Nodes 1, 2, 3
	// Cluster 2: Nodes 4, 5, 6
	
	for i := 1; i <= 6; i++ {
		db.DB.Exec("INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (?, ?, ?, ?, ?)", i, "Node", "Test", "q", "f")
	}

	insertEdge := func(s, t int) {
		db.DB.Exec("INSERT INTO relations (source_id, target_id, type) VALUES (?, ?, ?)", s, t, "CALLS")
	}

	// Clique 1
	insertEdge(1, 2)
	insertEdge(2, 3)
	insertEdge(3, 1)

	// Clique 2
	insertEdge(4, 5)
	insertEdge(5, 6)
	insertEdge(6, 4)

	// Run Leiden
	leiden := NewLeidenNative(1.0)
	err = leiden.Calculate(db)
	if err != nil {
		t.Fatalf("Leiden Calculate failed: %v", err)
	}

	// Verify clustering
	getComm := func(id int) int {
		var c int
		err := db.DB.QueryRow("SELECT community_id FROM entities WHERE id = ?", id).Scan(&c)
		if err != nil {
			t.Logf("Error fetching comm for %d: %v", id, err)
		}
		return c
	}

	c1, c2, c3 := getComm(1), getComm(2), getComm(3)
	c4, c5, c6 := getComm(4), getComm(5), getComm(6)

	t.Logf("Communities: %d %d %d | %d %d %d", c1, c2, c3, c4, c5, c6)

	// In disjoint cliques, they should have identical communities within cliques
	if c1 != c2 || c2 != c3 {
		t.Errorf("expected nodes 1,2,3 to be in same community, got %d, %d, %d", c1, c2, c3)
	}
	
	if c4 != c5 || c5 != c6 {
		t.Errorf("expected nodes 4,5,6 to be in same community, got %d, %d, %d", c4, c5, c6)
	}

	// And different communities between cliques
	if c1 == c4 {
		t.Errorf("expected disjoint cliques to end up in different communities, got both %d", c1)
	}
}
