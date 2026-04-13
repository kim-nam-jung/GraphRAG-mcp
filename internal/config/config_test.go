package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadConfig(t *testing.T) {
	yamlData := `
server:
  transport: "stdio"
indexer:
  tier1: ["go", "py"]
  exclude_dirs: ["vendor"]
  chunk_max_lines: 100
embedding:
  model_path: "path/to/model"
  dimension: 640
storage:
  db_path: "test.db"
  wal_mode: true
graph:
  leiden_resolution: 1.0
  min_community_size: 10
search:
  auto_reindex: true
`

	tmpDir := t.TempDir()
	cfgPath := filepath.Join(tmpDir, "test_config.yaml")

	err := os.WriteFile(cfgPath, []byte(yamlData), 0644)
	if err != nil {
		t.Fatalf("failed to create temp test yaml file: %v", err)
	}

	cfg, err := LoadConfig(cfgPath)
	if err != nil {
		t.Fatalf("expected to load config, got error: %v", err)
	}

	if cfg.Server.Transport != "stdio" {
		t.Errorf("expected Server.Transport stdio, got %s", cfg.Server.Transport)
	}
	if len(cfg.Indexer.Tier1) != 2 {
		t.Errorf("expected 2 Tier1 languages")
	}
	if cfg.Embedding.Dimension != 640 {
		t.Errorf("expected Embedding.Dimension 640, got %d", cfg.Embedding.Dimension)
	}
	if !cfg.Storage.WALMode {
		t.Errorf("expected Storage.WALMode true")
	}
	if cfg.Graph.LeidenResolution != 1.0 {
		t.Errorf("expected Graph.LeidenResolution 1.0")
	}
}

func TestLoadConfig_NotFound(t *testing.T) {
	_, err := LoadConfig("non_existent_file_39812.yaml")
	if err == nil {
		t.Errorf("expected error for missing config file")
	}
}
