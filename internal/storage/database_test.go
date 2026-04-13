package storage

import (
	"testing"
)

func TestInitDB(t *testing.T) {
	// Test the fallback to shared memory db
	db, err := InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("InitDB failed: %v", err)
	}
	defer db.Close()

	// Just a simple check to see if tables were created
	rows, err := db.DB.Query("SELECT name FROM sqlite_master WHERE type='table'")
	if err != nil {
		t.Fatalf("Failed to query basic tables: %v", err)
	}
	defer rows.Close()

	tableMaps := make(map[string]bool)
	for rows.Next() {
		var name string
		rows.Scan(&name)
		tableMaps[name] = true
	}

	requiredTables := []string{"entities", "relations", "chunks", "communities", "fts_entities", "fts_chunks", "vec_chunks"}
	for _, rt := range requiredTables {
		if !tableMaps[rt] {
			t.Errorf("Expected table %s to exist, but it was not found", rt)
		}
	}
}

func TestDatabase_Close(t *testing.T) {
	db, _ := InitDB(":memory:", true)
	err := db.Close()
	if err != nil {
		t.Errorf("Expected Clean Close, got %v", err)
	}
}
