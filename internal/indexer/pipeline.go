package indexer

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"os"

	"graphrag-mcp/internal/config"
	"graphrag-mcp/internal/embedding"
	"graphrag-mcp/internal/graph"
	"graphrag-mcp/internal/storage"
)

// floatsToBytes converts a float32 slice to a little-endian byte array (M1 Optimization)
func floatsToBytes(floats []float32) []byte {
	buf := new(bytes.Buffer)
	_ = binary.Write(buf, binary.LittleEndian, floats)
	return buf.Bytes()
}

// RunPipeline scans the given directory, dynamically loads extractors according to extensions,
// parses all files into ASTs, extracts entities/relations, and stores semantic embeddings in a transactional way.
func RunPipeline(path string, db *storage.Database, cfg *config.Config, harrier *embedding.HarrierModel, tokenizer *embedding.Tokenizer, parserReg *ParserRegistry) (int, int, error) {
	scanner := NewScanner(db, &cfg.Indexer)
	modifiedFiles, deletedFiles, err := scanner.Scan(path)
	if err != nil {
		return 0, 0, fmt.Errorf("scan error: %w", err)
	}

	for _, file := range modifiedFiles {
		content, err := os.ReadFile(file)
		if err != nil {
			continue
		}

		parser, err := parserReg.Get(file)
		if err != nil {
			continue // unsupported language
		}

		tree, err := parser.Parse(content)
		if err != nil {
			continue
		}

		entities, relations := parser.Extract(tree, content)
		maxChunks := 800
		if cfg.Indexer.ChunkMaxLines > 0 {
			maxChunks = cfg.Indexer.ChunkMaxLines
		}
		chunks := ChunkFile(content, entities, maxChunks)

		// Tx Begin for File isolation
		tx, err := db.DB.Begin()
		if err != nil {
			continue
		}

		// Pre-cleanup old file items (Review Issue 1-1)
		if _, err := tx.Exec("DELETE FROM entities WHERE file_path=?", file); err != nil {
			tx.Rollback()
			continue
		}
		// H3 Fix: manually delete vectors BEFORE chunks
		if _, err := tx.Exec("DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE file_path=?)", file); err != nil {
			tx.Rollback()
			continue
		}
		if _, err := tx.Exec("DELETE FROM chunks WHERE file_path=?", file); err != nil {
			tx.Rollback()
			continue
		}

		entityIDs := make(map[string]int64)

		// Insert Entities and Cache LastInsertId (Review Issue 1-2)
		entErr := false
		for _, ent := range entities {
			res, err := tx.Exec("INSERT OR IGNORE INTO entities (name, qualified_name, type, file_path, line_start, line_end) VALUES (?, ?, ?, ?, ?, ?)",
				ent.Name, ent.QualifiedName, ent.Type, file, ent.LineStart, ent.LineEnd)
			if err != nil {
				entErr = true
				break
			}
			id, err := res.LastInsertId()
			if err == nil && id > 0 {
				entityIDs[ent.Name] = id
			} else {
				// If IGNORE bypassed it, we fetch the row
				var existingID int64
				if tx.QueryRow("SELECT id FROM entities WHERE name=? AND file_path=?", ent.Name, file).Scan(&existingID) == nil {
					entityIDs[ent.Name] = existingID
				}
			}
		}
		if entErr {
			tx.Rollback()
			continue
		}

		// Insert Relations using entityIDs Cache
		relErr := false
		for _, rel := range relations {
			sourceID, ok := entityIDs[string(rel.Source)]
			if !ok {
				continue // Skip if unresolved local source
			}

			// Target might be external, so do subquery LIMIT 1
			_, err := tx.Exec("INSERT OR IGNORE INTO relations (source_id, target_id, type, weight) VALUES (?, (SELECT id FROM entities WHERE name=? LIMIT 1), ?, 1.0)",
				sourceID, string(rel.Target), rel.Type)
			if err != nil {
				relErr = true
				break
			}
		}
		if relErr {
			tx.Rollback()
			continue
		}

		// Insert Vector/Text Chunks
		chunkErr := false
		for _, chunk := range chunks {
			emb, err := harrier.Embed(chunk.Text, false, "", tokenizer)
			if err != nil {
				chunkErr = true
				break
			}

			var eid *int64
			if chunk.Entity != nil {
				if cachedId, ok := entityIDs[chunk.Entity.Name]; ok {
					eid = &cachedId
				}
			}

			res, err := tx.Exec("INSERT INTO chunks (text, file_path, line_start, line_end, entity_id) VALUES (?, ?, ?, ?, ?)",
				chunk.Text, file, chunk.LineStart, chunk.LineEnd, eid)
			if err != nil {
				chunkErr = true
				break
			}

			lastId, err := res.LastInsertId()
			if err == nil {
				// M1 Fix: Direct float32 slice to Blob without formatting Loss or JSON precision issues
				embBytes := floatsToBytes(emb)
				_, err = tx.Exec("INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?, ?)", lastId, embBytes)
				if err != nil {
					chunkErr = true
					break
				}
			}
		}

		if chunkErr {
			tx.Rollback()
			continue
		}

		tx.Commit()
	}

	// Calculate Leiden Communities explicitly after bulk inserts
	// Review Issue 1-3: ensure existing communities are deleted first
	db.DB.Exec("UPDATE entities SET community_id = NULL")
	db.DB.Exec("DELETE FROM communities")
	leiden := graph.NewLeidenNative(cfg.Graph.LeidenResolution)
	leiden.Calculate(db)

	return len(modifiedFiles), len(deletedFiles), nil
}
