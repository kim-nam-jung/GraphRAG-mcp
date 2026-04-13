package extractors

import (
	"testing"
)

func TestPythonExtractor_Extract(t *testing.T) {
	extractor := NewPythonExtractor()
	
	code := []byte(`
class MyClass:
    def method_one(self):
        pass

def global_function():
    pass
	`)

	tree, err := extractor.Parse(code)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	entities, relations := extractor.Extract(tree, code)
	
	var foundMethod bool
	var foundGlobal bool

	for _, ent := range entities {
		if ent.Name == "method_one" { foundMethod = true }
		if ent.Name == "global_function" { foundGlobal = true }
	}

	if !foundMethod {
		t.Errorf("expected to extract method 'method_one'")
	}
	if !foundGlobal {
		t.Errorf("expected to extract function 'global_function'")
	}

	t.Logf("Found %d entities, %d relations", len(entities), len(relations))
}
