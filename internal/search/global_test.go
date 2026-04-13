package search

import (
	"testing"
	"graphrag-mcp/internal/storage"
)

func TestGlobalSearch_Basic(t *testing.T) {
	db, err := storage.InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("failed to init db: %v", err)
	}
	defer db.Close()

	// Insert items into entities
	queries := []string{
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (1, 'LoginManager', 'class', 'q1', 'f1');",
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (2, 'LoginHelper', 'method', 'q2', 'f2');",
		"INSERT INTO entities (id, name, type, qualified_name, file_path) VALUES (3, 'HelperFunctions', 'class', 'q3', 'f3');",
	}

	for _, q := range queries {
		_, err := db.DB.Exec(q)
		if err != nil {
			t.Fatalf("failed to setup db: %v", err)
		}
	}

	// Insert edges to form relations
	_, err = db.DB.Exec("INSERT INTO relations (source_id, target_id, type) VALUES (1, 2, 'CONTAINS')")
	if err != nil {
		t.Fatalf("Failed to insert relation: %v", err)
	}

	// Debug FTS size
	var count int
	db.DB.QueryRow("SELECT COUNT(*) FROM fts_entities").Scan(&count)
	t.Logf("FTS Table count: %d", count)

	res, err := GlobalSearch(db, "Login*", 5)
	if err != nil {
		t.Fatalf("GlobalSearch failed: %v", err)
	}

	resMap := res.(map[string]interface{})
	entities := resMap["entities"].([]map[string]interface{})

	// Should match LoginManager and DoLogin
	if len(entities) != 2 {
		t.Errorf("expected 2 entities, got %d", len(entities))
	}

	// Entity 1 should contain a relationship to 'LoginHelper' based on our setup
	var foundRel bool
	for _, ent := range entities {
		rels := ent["relations"].([]map[string]string)
		for _, r := range rels {
			if r["target"] == "LoginHelper" {
				foundRel = true
			}
		}
	}

	if !foundRel {
	    // Log out what relations we actually had
	    t.Errorf("expected to capture relation, got entities: %v", entities)
	}
}
