package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"os"

	"graphrag-mcp/internal/config"
	"graphrag-mcp/internal/embedding"
	"graphrag-mcp/internal/mcp"
	"graphrag-mcp/internal/storage"
)

func main() {
	configPath := flag.String("config", "configs/default.yaml", "Path to config file")
	flag.Parse()

	// Load configuration
	cfg, err := config.LoadConfig(*configPath)
	if err != nil {
		log.Fatalf("Failed to load config from %s: %v", *configPath, err)
	}

	// Initialize GraphRAG Database
	db, err := storage.InitDB(cfg.Storage.DBPath, cfg.Storage.WALMode)
	if err != nil {
		log.Fatalf("Failed to initialize database: %v", err)
	}
	defer db.Close()

	// Initialize Models (Mocked loading for MVP)
	tokenizer, err := embedding.NewTokenizer(cfg.Embedding.TokenizerPath)
	if err != nil {
		log.Printf("Warning: failed to load tokenizer from %s: %v", cfg.Embedding.TokenizerPath, err)
	} else {
		defer tokenizer.Close()
	}

	harrier, err := embedding.NewHarrierModel(cfg.Embedding.ModelPath)
	if err != nil {
		log.Printf("Warning: failed to load harrier model from %s: %v", cfg.Embedding.ModelPath, err)
	} else {
		defer harrier.Close()
	}

	// Initialize and run MCP server
	server := mcp.NewServer(db, cfg, harrier, tokenizer)
	
	if err := server.Start(context.Background()); err != nil {
		fmt.Fprintf(os.Stderr, "Server error: %v\n", err)
		os.Exit(1)
	}
}
