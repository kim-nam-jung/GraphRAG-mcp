package extractors

import (
	"testing"
)

func TestGoExtractor_Extract(t *testing.T) {
	extractor := NewGoExtractor()
	
	code := []byte(`
package test

type MyClass struct {
	value string
}

func (m *MyClass) DoSomething() {
	// ...
}

func HelperFunc() {}
	`)

	tree, err := extractor.Parse(code)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	entities, relations := extractor.Extract(tree, code)
	
	// We expect: 
	// Entities: MyClass (type definitions or structs might be ignored depending on current logic, but let's check functions)
	// Actually, the current Go extractor maps "function_declaration" and "method_declaration"
	
	var foundDoSomething bool
	var foundHelperFunc bool

	for _, ent := range entities {
		if ent.Name == "DoSomething" { foundDoSomething = true }
		if ent.Name == "HelperFunc" { foundHelperFunc = true }
	}

	if !foundDoSomething {
		t.Errorf("expected to extract method 'DoSomething', but didn't find it")
	}
	if !foundHelperFunc {
		t.Errorf("expected to extract function 'HelperFunc', but didn't find it")
	}

	t.Logf("Found %d entities, %d relations", len(entities), len(relations))
}
