package mcp

import (
	"context"

	mcp_sdk "github.com/mark3labs/mcp-go/mcp"
	mcp_server "github.com/mark3labs/mcp-go/server"
	
	"graphrag-mcp/internal/config"
	"graphrag-mcp/internal/embedding"
	"graphrag-mcp/internal/indexer"
	"graphrag-mcp/internal/indexer/extractors"
	"graphrag-mcp/internal/storage"
	"sync"
	"time"
)

type Server struct {
	mcp       *mcp_server.MCPServer
	db        *storage.Database
	cfg       *config.Config
	harrier   *embedding.HarrierModel
	tokenizer *embedding.Tokenizer
	parserReg *indexer.ParserRegistry

	mu            sync.RWMutex
	lastScanPath  string
	lastIndexTime time.Time
}

// NewServer initializes the MCP server and registers tools
func NewServer(db *storage.Database, cfg *config.Config, harrier *embedding.HarrierModel, tokenizer *embedding.Tokenizer) *Server {
	mcpServer := mcp_server.NewMCPServer(
		"graphrag-mcp",
		"0.1.0",
		mcp_server.WithToolCapabilities(true),
	)

	// Initialize Parser Registry once (M3)
	parserReg := indexer.NewParserRegistry()
	parserReg.Register(".go", extractors.NewGoExtractor())
	parserReg.Register(".py", extractors.NewPythonExtractor())
	parserReg.Register(".js", extractors.NewJSExtractor())
	parserReg.Register(".ts", extractors.NewTSExtractor())
	parserReg.Register(".jsx", extractors.NewJSExtractor())
	parserReg.Register(".tsx", extractors.NewTSExtractor())

	s := &Server{
		mcp:       mcpServer,
		db:        db,
		cfg:       cfg,
		harrier:   harrier,
		tokenizer: tokenizer,
		parserReg: parserReg,
	}

	s.registerTools()

	return s
}

// Start runs the standard I/O serving loop for MCP
func (s *Server) Start(ctx context.Context) error {
	// Start stdio handles
	return mcp_server.ServeStdio(s.mcp)
}

func (s *Server) registerTools() {
	// Register Keyword Search
	keywordSearchTool := mcp_sdk.NewTool("keyword_search", 
		mcp_sdk.WithDescription("Provides FTS5 keyword search combined with graph context."),
		mcp_sdk.WithString("query", mcp_sdk.Required(), mcp_sdk.Description("Query or keyword to search")),
		mcp_sdk.WithNumber("top_k", mcp_sdk.Description("Maximum number of results to return")),
	)
	s.mcp.AddTool(keywordSearchTool, s.handleKeywordSearch)

	// Register Get Entity
	getEntityTool := mcp_sdk.NewTool("get_entity", 
		mcp_sdk.WithDescription("Returns detailed information, original source code, and relationships of a specific entity."),
		mcp_sdk.WithString("name", mcp_sdk.Required(), mcp_sdk.Description("Name of the entity")),
		mcp_sdk.WithString("file", mcp_sdk.Required(), mcp_sdk.Description("File path of the entity")),
	)
	s.mcp.AddTool(getEntityTool, s.handleGetEntity)

	// Register Global Search
	globalSearchTool := mcp_sdk.NewTool("global_search",
		mcp_sdk.WithDescription("Performs a broad search across all entities in the project and returns their relationships."),
		mcp_sdk.WithString("query", mcp_sdk.Description("Global search question or keyword (optional)")),
		mcp_sdk.WithNumber("max_entities", mcp_sdk.Description("Maximum number of entities to return")),
	)
	s.mcp.AddTool(globalSearchTool, s.handleGlobalSearch)

	// Register Graph Neighbors
	graphNeighborsTool := mcp_sdk.NewTool("graph_neighbors",
		mcp_sdk.WithDescription("Provides N-depth neighbor information for a specific entity."),
		mcp_sdk.WithString("entity", mcp_sdk.Required(), mcp_sdk.Description("Name of the entity")),
		mcp_sdk.WithNumber("depth", mcp_sdk.Description("Exploration depth N")),
		mcp_sdk.WithString("direction", mcp_sdk.Description("outgoing, incoming, both")),
	)
	s.mcp.AddTool(graphNeighborsTool, s.handleGraphNeighbors)

	// Register Index Directory
	indexDirTool := mcp_sdk.NewTool("index_directory",
		mcp_sdk.WithDescription("Performs incremental indexing and AST extraction by scanning the directory."),
		mcp_sdk.WithString("path", mcp_sdk.Required(), mcp_sdk.Description("Root path to scan")),
	)
	s.mcp.AddTool(indexDirTool, s.handleIndexDirectory)

	// Register Local Search
	localSearchTool := mcp_sdk.NewTool("local_search",
		mcp_sdk.WithDescription("Performs vector-based semantic code search and returns the surrounding N-Hop graph context along with code chunks."),
		mcp_sdk.WithString("query", mcp_sdk.Required(), mcp_sdk.Description("Natural language query")),
		mcp_sdk.WithNumber("top_k", mcp_sdk.Description("Number of initial vector similarity chunks to retrieve")),
		mcp_sdk.WithNumber("graph_depth", mcp_sdk.Description("Graph expansion depth")),
	)
	s.mcp.AddTool(localSearchTool, s.handleLocalSearch)
}
