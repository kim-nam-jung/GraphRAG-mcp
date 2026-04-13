package search

import (
	"testing"
	"graphrag-mcp/internal/storage"
)

func TestLocalSearch_NilModelHandling(t *testing.T) {
	db, err := storage.InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("failed to init db: %v", err)
	}
	defer db.Close()

	// If HarrierModel is nil, we should get panics or errors based on embedding logic.
	// We'll just verify the signature doesn't break.
	
	defer func() {
		if r := recover(); r != nil {
			t.Logf("Expected panic due to nil Harrier Model: %v", r)
		}
	}()

	_, _ = LocalSearch(db, nil, nil, "test", "", 5, 1)
	t.Errorf("Should have panicked on nil HarrierModel")
}
