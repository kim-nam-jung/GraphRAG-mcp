package extractors

import (
	"context"

	sitter "github.com/smacker/go-tree-sitter"
	"github.com/smacker/go-tree-sitter/javascript"
	"github.com/smacker/go-tree-sitter/typescript/typescript"
)

type JSExtractor struct {
	lang *sitter.Language
}

func NewJSExtractor() *JSExtractor {
	return &JSExtractor{lang: javascript.GetLanguage()}
}

func NewTSExtractor() *JSExtractor {
	return &JSExtractor{lang: typescript.GetLanguage()}
}

func (e *JSExtractor) Parse(content []byte) (*sitter.Tree, error) {
	parser := sitter.NewParser()
	parser.SetLanguage(e.lang)
	return parser.ParseCtx(context.Background(), nil, content)
}

func (e *JSExtractor) Extract(tree *sitter.Tree, content []byte) ([]Entity, []Relation) {
	var entities []Entity
	var relations []Relation

	// Query for functions, classes, and calls
	// JS/TS generic query
	q, err := sitter.NewQuery([]byte(`
		(function_declaration name: (identifier) @func.name) @func.def
		(class_declaration name: (type_identifier) @class.name) @class.def
		(method_definition name: (property_identifier) @method.name) @method.def
		(variable_declarator name: (identifier) @var.name value: (arrow_function)) @arrow.def
		(call_expression function: (identifier) @call.name) @call.expr
		(call_expression function: (member_expression property: (property_identifier) @call.name)) @call.expr
	`), e.lang)

	if err != nil {
		// Fallback for JS/TS type identifier differences
		q, _ = sitter.NewQuery([]byte(`
			(function_declaration name: (identifier) @func.name) @func.def
			(class_declaration name: (identifier) @class.name) @class.def
			(method_definition name: (property_identifier) @method.name) @method.def
			(variable_declarator name: (identifier) @var.name value: (arrow_function)) @arrow.def
			(call_expression function: (identifier) @call.name) @call.expr
			(call_expression function: (member_expression property: (property_identifier) @call.name)) @call.expr
		`), e.lang)
	}

	if q == nil {
		return entities, relations // Safe fallback if query parsing entirely fails
	}
	defer q.Close()

	qc := sitter.NewQueryCursor()
	qc.Exec(q, tree.RootNode())

	calls := make(map[int]string) // startLine -> called function

	for {
		m, ok := qc.NextMatch()
		if !ok {
			break
		}

		m = qc.FilterPredicates(m, content)
		var captureType string
		var name string
		var defNode *sitter.Node

		for _, c := range m.Captures {
			cName := q.CaptureNameForId(c.Index)
			if cName == "func.name" || cName == "class.name" || cName == "method.name" || cName == "var.name" {
				name = c.Node.Content(content)
			} else if cName == "func.def" {
				defNode = c.Node
				captureType = "function"
			} else if cName == "class.def" {
				defNode = c.Node
				captureType = "class"
			} else if cName == "method.def" {
				defNode = c.Node
				captureType = "method"
			} else if cName == "arrow.def" {
				defNode = c.Node
				captureType = "function"
			} else if cName == "call.name" {
				callName := c.Node.Content(content)
				calls[int(c.Node.StartPoint().Row)+1] = callName
			}
		}

		if name != "" && defNode != nil && captureType != "" {
			ent := Entity{
				Name:          name,
				QualifiedName: name, // MVP limit
				Type:          captureType,
				LineStart:     int(defNode.StartPoint().Row) + 1,
				LineEnd:       int(defNode.EndPoint().Row) + 1,
			}
			entities = append(entities, ent)
		}
	}

	// MVP Heuristic for relations (same as Go/Python)
	for lineStart, callName := range calls {
		for _, ent := range entities {
			if lineStart >= ent.LineStart && lineStart <= ent.LineEnd {
				relations = append(relations, Relation{
					Source: QualifiedName(ent.Name), // Function that contains the call
					Target: QualifiedName(callName), // Function being called
					Type:   "CALLS",
				})
				break
			}
		}
	}

	return entities, relations
}
