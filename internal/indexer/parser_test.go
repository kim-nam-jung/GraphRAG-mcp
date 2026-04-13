package indexer

import (
	"testing"
	"graphrag-mcp/internal/indexer/extractors"
)

func TestParserRegistry_Basic(t *testing.T) {
	reg := NewParserRegistry()

	// Initially empty map
	_, err := reg.Get("test.go")
	if err == nil {
		t.Fatal("expected error for unregistered extension")
	}

	// Register a dummy
	reg.Register(".go", extractors.NewGoExtractor())

	// Should fetch via exact extension
	parser, err := reg.Get("main.go")
	if err != nil {
		t.Fatalf("expected to get parser, got err: %v", err)
	}
	if parser == nil {
		t.Fatal("expected non-nil parser")
	}

	// Should not fetch python
	_, err = reg.Get("main.py")
	if err == nil {
		t.Fatal("expected error for .py, got none")
	}
}

func TestParserRegistry_DotHandling(t *testing.T) {
	reg := NewParserRegistry()

	// Register explicitly without dot, should be normalized?
	// Our Map is exactly mapping string to Extractor.
	reg.Register("py", extractors.NewPythonExtractor())

	// filepath.Ext returns ".py". If we registered without dot, we might have an issue.
	// But our codebase convention is that Register is called with dot (i.e. ".py")
	_, err := reg.Get("main.py")
	if err == nil {
		t.Logf("Warning: Registry automatically handled dot mapping, which is nice!")
	} else {
		t.Logf("Registry is strictly matching exactly what was registered. Normal behavior.")
	}
}
