package search

import (
	"database/sql"
	"fmt"

	"graphrag-mcp/internal/storage"
)

// GlobalSearch searches across all indexed entities in the DB
func GlobalSearch(db *storage.Database, query string, maxEntities int) (interface{}, error) {
	var rows *sql.Rows
	var err error

	if query == "" {
		// Just retrieve random/top entities if no query is given, e.g., the root modules
		q := `SELECT id, name, qualified_name, type, file_path FROM entities LIMIT ?`
		rows, err = db.DB.Query(q, maxEntities)
	} else {
		// Use FTS to match entities across the entire DB
		q := `
			SELECT e.id, e.name, e.qualified_name, e.type, e.file_path
			FROM fts_entities fts
			JOIN entities e ON fts.rowid = e.id
			WHERE fts_entities MATCH ?
			LIMIT ?
		`
		rows, err = db.DB.Query(q, query, maxEntities)
	}

	if err != nil {
		return nil, fmt.Errorf("failed to search entities: %w", err)
	}
	defer rows.Close()

	var result []map[string]interface{}
	for rows.Next() {
		var id int
		var name, qName, eType, fPath string
		if err := rows.Scan(&id, &name, &qName, &eType, &fPath); err != nil {
			continue
		}

		// fetch some relationships from this entity (up to 5 to give structural context)
		relQ := `
			SELECT e_t.name, r.type
			FROM relations r
			JOIN entities e_t ON r.target_id = e_t.id
			WHERE r.source_id = ?
			LIMIT 5
		`
		erows, err := db.DB.Query(relQ, id)
		var relations []map[string]string
		if err == nil {
			for erows.Next() {
				var targetName, relType string
				if err := erows.Scan(&targetName, &relType); err == nil {
					relations = append(relations, map[string]string{
						"target": targetName,
						"type":   relType,
					})
				} else {
					fmt.Printf("SCAN ERROR RELATION: %v\n", err)
				}
			}
			erows.Close()
		} else {
			fmt.Printf("QUERY ERROR RELATION: %v\n", err)
		}

		entityData := map[string]interface{}{
			"id":             id,
			"name":           name,
			"qualified_name": qName,
			"type":           eType,
			"file_path":      fPath,
			"relations":      relations,
		}
		result = append(result, entityData)
	}

	return map[string]interface{}{
		"entities": result,
	}, nil
}
