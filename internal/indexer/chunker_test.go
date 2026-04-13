package indexer

import (
	"testing"
	"graphrag-mcp/internal/indexer/extractors"
)

func TestChunkFile_Basic(t *testing.T) {
	content := []byte("line1\nline2\nline3\nline4\nline5\nline6")
	
	// Create a fake entity that covers line 2 to 3
	entities := []extractors.Entity{
		{
			Name: "TestEntity",
			LineStart: 2,
			LineEnd: 3,
		},
	}

	chunks := ChunkFile(content, entities, 2)
	
	// Chunk 1: entity chunk covering line 2-3
	// Chunk 2: fallback chunk covering line 1
	// Chunk 3: fallback chunk covering line 4-5 (maxLines 2)
	// Chunk 4: fallback chunk covering line 6
	
	if len(chunks) != 4 {
		t.Fatalf("expected 4 chunks, got %d", len(chunks))
	}

	var hasEntity, hasLine1, hasLine45, hasLine6 bool
	for _, c := range chunks {
		if c.LineStart == 2 && c.LineEnd == 3 && c.Entity != nil {
			hasEntity = true
		} else if c.LineStart == 1 && c.LineEnd == 1 {
			hasLine1 = true
		} else if c.LineStart == 4 && c.LineEnd == 5 {
			hasLine45 = true
		} else if c.LineStart == 6 && c.LineEnd == 6 {
			hasLine6 = true
		}
	}

	if !hasEntity || !hasLine1 || !hasLine45 || !hasLine6 {
		t.Errorf("missing expected chunks! Chunks: %v", chunks)
	}
}

func TestChunkFile_EmptyFallback(t *testing.T) {
	// Let's test if strings.TrimSpace removes empty fallbacks
	content := []byte("\n\n\nline4\n\n")
	
	entities := []extractors.Entity{
		{
			Name: "TestEntity",
			LineStart: 4,
			LineEnd: 4,
		},
	}

	chunks := ChunkFile(content, entities, 10)
	
	// Lines 1,2,3 are empty. 4 is entity. 5,6 are empty.
	// Empty fallback chunks should be trimmed!
	if len(chunks) != 1 {
		t.Fatalf("expected 1 chunk (the entity), got %d", len(chunks))
	}

	if chunks[0].LineStart != 4 || chunks[0].LineEnd != 4 {
		t.Errorf("expected only the entity chunk at line 4, got: %v", chunks[0])
	}
}
