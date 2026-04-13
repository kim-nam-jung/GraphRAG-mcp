package extractors

import (
	"context"

	sitter "github.com/smacker/go-tree-sitter"
	"github.com/smacker/go-tree-sitter/python"
)

type PythonExtractor struct{}

func NewPythonExtractor() *PythonExtractor {
	return &PythonExtractor{}
}

func (e *PythonExtractor) Parse(content []byte) (*sitter.Tree, error) {
	parser := sitter.NewParser()
	parser.SetLanguage(python.GetLanguage())
	return parser.ParseCtx(context.Background(), nil, content)
}

func (e *PythonExtractor) Extract(tree *sitter.Tree, content []byte) ([]Entity, []Relation) {
	var entities []Entity
	var relations []Relation

	// Query for Python entities
	q, err := sitter.NewQuery([]byte(`
		(function_definition name: (identifier) @func.name) @func.def
		(class_definition name: (identifier) @class.name) @class.def
		(call function: (identifier) @call.name) @call.def
	`), python.GetLanguage())
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
			if name == "func.name" || name == "class.name" || name == "call.name" {
				qName = c.Node.Content(content)
			}
			if name == "func.def" {
				nodeType = TypeFunction
				start = int(c.Node.StartPoint().Row) + 1
				end = int(c.Node.EndPoint().Row) + 1
			} else if name == "class.def" {
				nodeType = TypeClass
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
					QualifiedName: qName,
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
