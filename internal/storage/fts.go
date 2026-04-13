package storage

import (
	"database/sql"
	"fmt"
)

// KeywordResult represents the exact return structure for the MCP tool
type KeywordResult struct {
	Name    string `json:"name"`
	Type    string `json:"type"`
	File    string `json:"file"`
	Lines   string `json:"lines"`
	Snippet string `json:"snippet"`
}

// SearchFTS queries the FTS5 virtual tables based on keyword
func (d *Database) SearchFTS(query string, topK int) ([]KeywordResult, error) {
	// A basic implementation merging entities and chunks
	// To match the requirements exactly:
	// We lookup chunks containing the keyword, and join with entities to get name/type.
	// If a chunk lacks an entity_id, it is a fallback chunk and we represent it differently.

	q := `
SELECT 
    COALESCE(e.name, '') as name,
    COALESCE(e.type, 'FILE_CHUNK') as type,
    c.file_path,
    c.line_start,
    c.line_end,
    snippet(fts_chunks, 0, '<b>', '</b>', '...', 10) as snippet
FROM fts_chunks f
JOIN chunks c ON f.rowid = c.id
LEFT JOIN entities e ON c.entity_id = e.id
WHERE f.text MATCH ?
ORDER BY rank
LIMIT ?
`
	rows, err := d.DB.Query(q, query, topK)
	if err != nil {
		return nil, fmt.Errorf("FTS query failed: %w", err)
	}
	defer rows.Close()

	var results []KeywordResult
	for rows.Next() {
		var r KeywordResult
		var start, end sql.NullInt64 // use sql.NullInt64 directly
		var snippet string

		if err := rows.Scan(&r.Name, &r.Type, &r.File, &start, &end, &snippet); err != nil {
			return nil, err
		}

		if start.Valid && end.Valid {
			r.Lines = fmt.Sprintf("%d-%d", start.Int64, end.Int64)
		} else {
			r.Lines = "-"
		}
		r.Snippet = snippet
		results = append(results, r)
	}

	return results, nil
}
