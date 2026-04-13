package mcp

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	mcp_sdk "github.com/mark3labs/mcp-go/mcp"
	
	"graphrag-mcp/internal/search"
	"graphrag-mcp/internal/indexer"
)


// handleKeywordSearch processes the "keyword_search" tool call
func (s *Server) handleKeywordSearch(ctx context.Context, request mcp_sdk.CallToolRequest) (*mcp_sdk.CallToolResult, error) {
	args, ok := request.Params.Arguments.(map[string]interface{})
	if !ok {
		return mcp_sdk.NewToolResultError("invalid arguments map"), nil
	}
	
	query, ok := args["query"].(string)
	if !ok || query == "" {
		return mcp_sdk.NewToolResultError("query argument is mandatory"), nil
	}

	topK := 10
	if val, ok := args["top_k"].(float64); ok && val > 0 {
		topK = int(val)
	}

	results, err := s.db.SearchFTS(query, topK)
	if err != nil {
		return mcp_sdk.NewToolResultError(fmt.Sprintf("FTS search error: %v", err)), nil
	}

	responseWrapper := map[string]interface{}{
		"results": results,
	}

	data, _ := json.Marshal(responseWrapper)

	return mcp_sdk.NewToolResultText(string(data)), nil
}

// handleGetEntity processes the "get_entity" tool call
func (s *Server) handleGetEntity(ctx context.Context, request mcp_sdk.CallToolRequest) (*mcp_sdk.CallToolResult, error) {
	args, ok := request.Params.Arguments.(map[string]interface{})
	if !ok {
		return mcp_sdk.NewToolResultError("invalid arguments map"), nil
	}

	name, ok := args["name"].(string)
	if !ok || name == "" {
		return mcp_sdk.NewToolResultError("name argument is mandatory"), nil
	}

	file, ok := args["file"].(string)
	if !ok || file == "" {
		return mcp_sdk.NewToolResultError("file argument is mandatory"), nil
	}

	res, err := s.db.GetEntity(name, file)
	if err != nil {
		return mcp_sdk.NewToolResultError(fmt.Sprintf("Failed to get entity: %v", err)), nil
	}

	data, _ := json.Marshal(res)

	return mcp_sdk.NewToolResultText(string(data)), nil
}

func (s *Server) handleGlobalSearch(ctx context.Context, request mcp_sdk.CallToolRequest) (*mcp_sdk.CallToolResult, error) {
	args, ok := request.Params.Arguments.(map[string]interface{})
	if !ok {
		return mcp_sdk.NewToolResultError("invalid arguments map"), nil
	}

	query, _ := args["query"].(string)
	maxEntities := 50
	if val, ok := args["max_entities"].(float64); ok && val > 0 {
		maxEntities = int(val)
	}

	res, err := search.GlobalSearch(s.db, query, maxEntities)
	if err != nil {
		return mcp_sdk.NewToolResultError(fmt.Sprintf("Global search error: %v", err)), nil
	}

	data, _ := json.Marshal(res)
	return mcp_sdk.NewToolResultText(string(data)), nil
}

func (s *Server) handleGraphNeighbors(ctx context.Context, request mcp_sdk.CallToolRequest) (*mcp_sdk.CallToolResult, error) {
	args, ok := request.Params.Arguments.(map[string]interface{})
	if !ok {
		return mcp_sdk.NewToolResultError("invalid arguments map"), nil
	}

	entity, ok := args["entity"].(string)
	if !ok || entity == "" {
		return mcp_sdk.NewToolResultError("entity argument is mandatory"), nil
	}

	depth := 1
	if dval, ok := args["depth"].(float64); ok && dval > 0 {
		depth = int(dval)
	}

	direction := "outgoing"
	if dirval, ok := args["direction"].(string); ok && dirval != "" {
		direction = dirval
	}

	res, err := search.GraphNeighbors(s.db, entity, depth, direction)
	if err != nil {
		return mcp_sdk.NewToolResultError(fmt.Sprintf("Graph neighbors error: %v", err)), nil
	}

	data, _ := json.Marshal(res)
	return mcp_sdk.NewToolResultText(string(data)), nil
}

// Removed runIndexing, logic moved to internal/indexer/RunsPipeline

func (s *Server) handleIndexDirectory(ctx context.Context, request mcp_sdk.CallToolRequest) (*mcp_sdk.CallToolResult, error) {
	args, ok := request.Params.Arguments.(map[string]interface{})
	if !ok {
		return mcp_sdk.NewToolResultError("invalid arguments map"), nil
	}

	path, ok := args["path"].(string)
	if !ok || path == "" {
		return mcp_sdk.NewToolResultError("path argument is mandatory"), nil
	}

	s.mu.Lock()
	s.lastScanPath = path
	s.mu.Unlock()

	modified, deleted, err := indexer.RunPipeline(path, s.db, s.cfg, s.harrier, s.tokenizer, s.parserReg)
	if err != nil {
		return mcp_sdk.NewToolResultError(err.Error()), nil
	}

	res := map[string]interface{}{
		"modified": modified,
		"deleted":  deleted,
		"status":   "Index Eager processing completed",
	}

	data, _ := json.Marshal(res)
	return mcp_sdk.NewToolResultText(string(data)), nil
}

func (s *Server) handleLocalSearch(ctx context.Context, request mcp_sdk.CallToolRequest) (*mcp_sdk.CallToolResult, error) {
	args, ok := request.Params.Arguments.(map[string]interface{})
	if !ok {
		return mcp_sdk.NewToolResultError("invalid arguments map"), nil
	}

	query, ok := args["query"].(string)
	if !ok || query == "" {
		return mcp_sdk.NewToolResultError("query argument is mandatory"), nil
	}

	topK := 5
	if val, ok := args["top_k"].(float64); ok && val > 0 {
		topK = int(val)
	}

	graphDepth := 1
	if val, ok := args["graph_depth"].(float64); ok && val >= 0 {
		graphDepth = int(val)
	}

	// Auto-Reindex 훅 구현
	if s.cfg.Search.AutoReindex {
		s.mu.Lock()
		path := s.lastScanPath
		lastTime := s.lastIndexTime
		s.mu.Unlock()

		if path != "" && time.Since(lastTime).Seconds() > float64(s.cfg.Search.ReindexCooldownSec) {
			_, _, err := indexer.RunPipeline(path, s.db, s.cfg, s.harrier, s.tokenizer, s.parserReg)
			if err == nil {
				s.mu.Lock()
				s.lastIndexTime = time.Now()
				s.mu.Unlock()
			}
		}
	}

	res, err := search.LocalSearch(s.db, s.harrier, s.tokenizer, query, s.cfg.Embedding.QueryInstruction, topK, graphDepth)
	if err != nil {
		return mcp_sdk.NewToolResultError(fmt.Sprintf("Local search error: %v", err)), nil
	}

	data, _ := json.Marshal(res)
	return mcp_sdk.NewToolResultText(string(data)), nil
}
