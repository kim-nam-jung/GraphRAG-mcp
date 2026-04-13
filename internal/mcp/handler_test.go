package mcp

import (
	"testing"

	"graphrag-mcp/internal/config"
	"graphrag-mcp/internal/embedding"
	"graphrag-mcp/internal/storage"
)

func TestNewGraphRAGServer(t *testing.T) {
	// Start with mock config
	cfg := &config.Config{
		Server: config.ServerConfig{Transport: "stdio"},
	}

	// Mock DB
	db, err := storage.InitDB(":memory:", false)
	if err != nil {
		t.Fatalf("Failed to init DB: %v", err)
	}
	defer db.Close()

	// Mock Harrier
	harrier, _ := embedding.NewHarrierModel("dummy")

	srv := NewServer(db, cfg, harrier, nil)

	if srv == nil {
		t.Fatalf("expected server instance, got nil")
	}
	if srv.mcp == nil {
		t.Fatalf("expected inner MCP server to be initialized")
	}

	// Just checking if basic tools are registered
	// We send a list_tools request directly to the server's router if possible,
	// but Mark3Labs MCP server hides the router.
	// We just ensure creation didn't panic.
}

func TestServer_ToolsPanic(t *testing.T) {
	// Testing the handler functions without a full MCP client is tricky because
	// mcp-go focuses on STDIO streams. So we test direct struct instantiation.
	cfg := &config.Config{}
	
	defer func() {
		if r := recover(); r != nil {
			t.Errorf("Server initialization panicked: %v", r)
		}
	}()

	NewServer(nil, cfg, nil, nil)
}
