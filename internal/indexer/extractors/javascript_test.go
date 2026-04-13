package extractors

import (
	"testing"
)

func TestJavascriptExtractor_Extract(t *testing.T) {
	// Tests JS
	extractor := NewJSExtractor()
	
	code := []byte(`
class User {
	login() {}
}

function globalJs() {}
	`)

	tree, err := extractor.Parse(code)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	entities, relations := extractor.Extract(tree, code)
	
	var foundJs bool
	for _, ent := range entities {
		if ent.Name == "globalJs" { foundJs = true }
	}

	if !foundJs {
		t.Errorf("expected to extract function 'globalJs' from JS")
	}
	t.Logf("JS: Found %d entities, %d relations", len(entities), len(relations))
}

func TestTypescriptExtractor_Extract(t *testing.T) {
	// Tests TS
	extractor := NewTSExtractor()
	
	code := []byte(`
interface MyInterface {}
class User implements MyInterface {
	public login(): void {}
}

const globalTs = () => {};
function doSomething(): string { return ""; }
	`)

	tree, err := extractor.Parse(code)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	entities, relations := extractor.Extract(tree, code)
	
	var foundTs bool
	for _, ent := range entities {
		if ent.Name == "doSomething" { foundTs = true }
	}

	if !foundTs {
		t.Errorf("expected to extract function 'doSomething' from TS")
	}
	t.Logf("TS: Found %d entities, %d relations", len(entities), len(relations))
}
