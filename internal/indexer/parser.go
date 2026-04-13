package indexer

import (
	"fmt"
	"path/filepath"

	"graphrag-mcp/internal/indexer/extractors"
)

// ParserRegistry acts as a factory for language-specific Extractors
type ParserRegistry struct {
	extractors map[string]extractors.Extractor
}

func NewParserRegistry() *ParserRegistry {
	return &ParserRegistry{
		extractors: make(map[string]extractors.Extractor),
	}
}

// Register maps a file extension (e.g., ".go") to its structural extractor
func (r *ParserRegistry) Register(ext string, extImpl extractors.Extractor) {
	r.extractors[ext] = extImpl
}

// Get finds an extractor for the given file path based on its extension
func (r *ParserRegistry) Get(filePath string) (extractors.Extractor, error) {
	ext := filepath.Ext(filePath)
	if impl, ok := r.extractors[ext]; ok {
		return impl, nil
	}
	return nil, fmt.Errorf("no extractor registered for extension: %s", ext)
}
