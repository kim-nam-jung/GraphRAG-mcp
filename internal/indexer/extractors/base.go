package extractors

import sitter "github.com/smacker/go-tree-sitter"

// EntityType matches the DB 'type' column
const (
	TypeFile      = "FILE"
	TypeFunction  = "FUNCTION"
	TypeMethod    = "METHOD"
	TypeClass     = "CLASS"
	TypeInterface = "INTERFACE"
	TypeVariable  = "VARIABLE"
	TypeConstant  = "CONSTANT"
	TypeType      = "TYPE"
	TypeStruct    = "STRUCT"
)

// RelationType matches the DB 'type' column
const (
	RelContains   = "CONTAINS"
	RelImports    = "IMPORTS"
	RelCalls      = "CALLS"
	RelHasField   = "HAS_FIELD"
	RelImplements = "IMPLEMENTS"
	RelInherits   = "INHERITS"
	RelReturns    = "RETURNS"
	RelReceives   = "RECEIVES"
)

type Entity struct {
	Name          string
	QualifiedName string
	Type          string
	LineStart     int
	LineEnd       int
}

type Relation struct {
	Source QualifiedName
	Target QualifiedName
	Type   string
}

type QualifiedName string

// Extractor corresponds to a specific programming language parser and structural extractor
type Extractor interface {
	// Parse translates source byte code into an AST
	Parse(content []byte) (*sitter.Tree, error)
	// Extract navigates the AST and extracts entities and relations
	Extract(tree *sitter.Tree, content []byte) ([]Entity, []Relation)
}
