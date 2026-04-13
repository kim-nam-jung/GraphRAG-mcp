package storage

import (
	"encoding/json"
	"fmt"
)

// WriteChunkEmbedding inserts or updates a chunk's vector embedding
func (d *Database) WriteChunkEmbedding(chunkID int64, embedding []float32) error {
	// We need to pass the float32 slice as JSON or a blob depending on the sqlite-vec API
	// The standard way with vec0 in Go using normal sql/driver is often JSON array cast to vec_f32()
	// assuming github.com/asg017/sqlite-vec-go-bindings loaded via Auto()
	
	embBytes, err := json.Marshal(embedding)
	if err != nil {
		return err
	}

	// upsert into sqlite-vec
	query := `INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?, ?) 
		ON CONFLICT(chunk_id) DO UPDATE SET embedding=excluded.embedding`
		
	_, err = d.DB.Exec(query, chunkID, string(embBytes))
	return err
}

// SearchSimilarChunks searches for chunks nearest to the provided embedding
func (d *Database) SearchSimilarChunks(embedding []float32, topK int) ([]int64, []float32, error) {
	embBytes, err := json.Marshal(embedding)
	if err != nil {
		return nil, nil, err
	}

	query := `
SELECT 
    chunk_id, 
    distance 
FROM vec_chunks 
WHERE embedding MATCH ? 
ORDER BY distance 
LIMIT ?`

	rows, err := d.DB.Query(query, string(embBytes), topK)
	if err != nil {
		return nil, nil, fmt.Errorf("vector search failed: %w", err)
	}
	defer rows.Close()

	var chunkIDs []int64
	var distances []float32
	for rows.Next() {
		var id int64
		var dist float32
		if err := rows.Scan(&id, &dist); err != nil {
			return nil, nil, err
		}
		chunkIDs = append(chunkIDs, id)
		distances = append(distances, dist)
	}

	return chunkIDs, distances, nil
}
