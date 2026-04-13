package extractors

import (
	"context"

	sitter "github.com/smacker/go-tree-sitter"
	"github.com/smacker/go-tree-sitter/golang"
)

type GoExtractor struct{}

func NewGoExtractor() *GoExtractor {
	return &GoExtractor{}
}

func (e *GoExtractor) Parse(content []byte) (*sitter.Tree, error) {
	parser := sitter.NewParser()
	parser.SetLanguage(golang.GetLanguage())
	return parser.ParseCtx(context.Background(), nil, content)
}

func (e *GoExtractor) Extract(tree *sitter.Tree, content []byte) ([]Entity, []Relation) {
	var entities []Entity
	var relations []Relation

	// Query for Go entities and relations
	q, err := sitter.NewQuery([]byte(`
		(function_declaration name: (identifier) @func.name) @func.def
		(method_declaration name: (field_identifier) @method.name) @method.def
		(type_spec name: (type_identifier) @type.name) @type.def
		(call_expression function: (identifier) @call.name) @call.def
	`), golang.GetLanguage())
	if err != nil {
		return entities, relations
	}

	qc := sitter.NewQueryCursor()
	qc.Exec(q, tree.RootNode())

	type callInfo struct {
		Name string
		Line int
	}
	var calls []callInfo

	for {
		m, ok := qc.NextMatch()
		if !ok {
			break
		}
		
		qName := ""
		nodeType := ""
		start := 0
		end := 0

		for _, c := range m.Captures {
			name := q.CaptureNameForId(c.Index)
			if name == "func.name" || name == "method.name" || name == "type.name" || name == "call.name" {
				qName = c.Node.Content(content)
			}
			if name == "func.def" {
				nodeType = TypeFunction
				start = int(c.Node.StartPoint().Row) + 1
				end = int(c.Node.EndPoint().Row) + 1
			} else if name == "method.def" {
				nodeType = TypeMethod
				start = int(c.Node.StartPoint().Row) + 1
				end = int(c.Node.EndPoint().Row) + 1
			} else if name == "type.def" {
				nodeType = TypeType
				start = int(c.Node.StartPoint().Row) + 1
				end = int(c.Node.EndPoint().Row) + 1
			} else if name == "call.def" {
				nodeType = "CALL"
				start = int(c.Node.StartPoint().Row) + 1
			}
		}

		if qName != "" && nodeType != "" {
			if nodeType == "CALL" {
				calls = append(calls, callInfo{Name: qName, Line: start})
			} else {
				entities = append(entities, Entity{
					Name:          qName,
					QualifiedName: qName, // Needs full resolution in advanced parsers
					Type:          nodeType,
					LineStart:     start,
					LineEnd:       end,
				})
			}
		}
	}

	// Heuristic relation mapping
	for _, call := range calls {
		for _, ent := range entities {
			if call.Line >= ent.LineStart && call.Line <= ent.LineEnd {
				relations = append(relations, Relation{
					Source: QualifiedName(ent.QualifiedName),
					Target: QualifiedName(call.Name),
					Type:   RelCalls,
				})
				break
			}
		}
	}

	return entities, relations
}
