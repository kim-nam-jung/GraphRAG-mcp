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
		mcp_sdk.WithDescription("FTS5 키워드 검색 + 그래프 컨텍스트를 제공합니다."),
		mcp_sdk.WithString("query", mcp_sdk.Required(), mcp_sdk.Description("검색할 쿼리/키워드")),
		mcp_sdk.WithNumber("top_k", mcp_sdk.Description("반환할 최대 결과 수")),
	)
	s.mcp.AddTool(keywordSearchTool, s.handleKeywordSearch)

	// Register Get Entity
	getEntityTool := mcp_sdk.NewTool("get_entity", 
		mcp_sdk.WithDescription("특정 엔티티의 상세 정보 + 코드 원문 + 관계를 반환합니다."),
		mcp_sdk.WithString("name", mcp_sdk.Required(), mcp_sdk.Description("엔티티 이름")),
		mcp_sdk.WithString("file", mcp_sdk.Required(), mcp_sdk.Description("엔티티 파일 경로")),
	)
	s.mcp.AddTool(getEntityTool, s.handleGetEntity)

	// Register Global Search
	globalSearchTool := mcp_sdk.NewTool("global_search",
		mcp_sdk.WithDescription("프로젝트 내의 모든 엔티티를 광범위하게 검색하고 관계망을 반환합니다."),
		mcp_sdk.WithString("query", mcp_sdk.Description("전역 검색 질문 또는 키워드 (선택)")),
		mcp_sdk.WithNumber("max_entities", mcp_sdk.Description("최대 엔티티 반환 수")),
	)
	s.mcp.AddTool(globalSearchTool, s.handleGlobalSearch)

	// Register Graph Neighbors
	graphNeighborsTool := mcp_sdk.NewTool("graph_neighbors",
		mcp_sdk.WithDescription("특정 엔티티의 N-depth 이웃 정보를 제공합니다."),
		mcp_sdk.WithString("entity", mcp_sdk.Required(), mcp_sdk.Description("엔티티 이름")),
		mcp_sdk.WithNumber("depth", mcp_sdk.Description("탐색 깊이 N")),
		mcp_sdk.WithString("direction", mcp_sdk.Description("outgoing, incoming, both")),
	)
	s.mcp.AddTool(graphNeighborsTool, s.handleGraphNeighbors)

	// Register Index Directory
	indexDirTool := mcp_sdk.NewTool("index_directory",
		mcp_sdk.WithDescription("디렉토리 스캔을 통해 증분 인덱싱/AST 추출 작업을 수행합니다."),
		mcp_sdk.WithString("path", mcp_sdk.Required(), mcp_sdk.Description("스캔할 루트 경로")),
	)
	s.mcp.AddTool(indexDirTool, s.handleIndexDirectory)

	// Register Local Search
	localSearchTool := mcp_sdk.NewTool("local_search",
		mcp_sdk.WithDescription("벡터 의미 기반 코드 검색 후 주변 N-Hop 그래프 문맥과 코드 청크를 함께 반환합니다."),
		mcp_sdk.WithString("query", mcp_sdk.Required(), mcp_sdk.Description("자연어 쿼리")),
		mcp_sdk.WithNumber("top_k", mcp_sdk.Description("초기 찾을 벡터 유사도 청크 개수")),
		mcp_sdk.WithNumber("graph_depth", mcp_sdk.Description("연결망 확장 깊이")),
	)
	s.mcp.AddTool(localSearchTool, s.handleLocalSearch)
}
