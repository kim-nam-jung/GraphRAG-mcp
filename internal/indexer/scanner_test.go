package indexer

import (
	"os"
	"path/filepath"
	"testing"

	"graphrag-mcp/internal/config"
	"graphrag-mcp/internal/storage"
)

func TestScanner_ExtensionHandling(t *testing.T) {
	// Create an in-memory test database
	db, err := storage.InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("failed to init in-memory db: %v", err)
	}
	defer db.Close()

	// Temporary directory for mock files
	tempDir := t.TempDir()

	// Create test files
	validFile1 := filepath.Join(tempDir, "main.go")
	validFile2 := filepath.Join(tempDir, "script.py")
	invalidFile := filepath.Join(tempDir, "data.txt")

	for _, f := range []string{validFile1, validFile2, invalidFile} {
		if err := os.WriteFile(f, []byte("test"), 0644); err != nil {
			t.Fatalf("failed to create temp file %s: %v", f, err)
		}
	}

	// Deliberately omit dots to test the M4 bugfix
	cfg := &config.IndexerConfig{
		Tier1: []string{"go"}, 
		Tier2: []string{".py"}, // some with dot, some without
		Tier3: []string{},
	}

	scanner := NewScanner(db, cfg)
	modified, deleted, err := scanner.Scan(tempDir)
	if err != nil {
		t.Fatalf("scanner.Scan returned error: %v", err)
	}

	if len(deleted) != 0 {
		t.Errorf("expected 0 deleted files, got %d", len(deleted))
	}

	if len(modified) != 2 {
		t.Errorf("expected exactly 2 modified files (.go, .py), got %d: %v", len(modified), modified)
	}

	// Verify only valid files were caught
	foundGo := false
	foundPy := false
	for _, f := range modified {
		if f == validFile1 {
			foundGo = true
		}
		if f == validFile2 {
			foundPy = true
		}
		if f == invalidFile {
			t.Errorf("scanner incorrectly picked up invalid text file: %s", f)
		}
	}

	if !foundGo || !foundPy {
		t.Errorf("failed to scan both target language files")
	}
}
