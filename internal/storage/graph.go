package storage

import (
	"database/sql"
	"fmt"
	"strings"
)

type EntityResult struct {
	Name        string `json:"name"`
	Type        string `json:"type"`
	Lines       string `json:"lines"`
	CommunityID int    `json:"community_id"`
}

type RelationResult struct {
	Type   string `json:"type"`
	Target string `json:"target,omitempty"`
	Source string `json:"source,omitempty"`
}

type GetEntityResult struct {
	Entity    EntityResult                `json:"entity"`
	Relations map[string][]RelationResult `json:"relations"`
	Code      string                      `json:"code"`
}

// GetEntity returns detailed information for a specific entity
func (d *Database) GetEntity(name, file string) (*GetEntityResult, error) {
	// Query the entity
	queryEntity := `
SELECT id, name, type, CASE WHEN line_start IS NULL THEN '' ELSE line_start || '-' || line_end END, COALESCE(community_id, -1)
FROM entities
WHERE name = ? AND file_path = ?
LIMIT 1`

	var entityID int64
	var res GetEntityResult
	var lines sql.NullString
	var commID sql.NullInt64

	err := d.DB.QueryRow(queryEntity, name, file).Scan(&entityID, &res.Entity.Name, &res.Entity.Type, &lines, &commID)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("entity not found")
		}
		return nil, err
	}

	res.Entity.Lines = lines.String
	if commID.Valid && commID.Int64 >= 0 {
		res.Entity.CommunityID = int(commID.Int64)
	}

	// Query outgoing relations
	queryOutgoing := `
SELECT r.type, e.name
FROM relations r
JOIN entities e ON r.target_id = e.id
WHERE r.source_id = ?`

	res.Relations = make(map[string][]RelationResult)
	res.Relations["outgoing"] = make([]RelationResult, 0)
	
	rowsOut, err := d.DB.Query(queryOutgoing, entityID)
	if err == nil {
		defer rowsOut.Close()
		for rowsOut.Next() {
			var rtype, target string
			if err := rowsOut.Scan(&rtype, &target); err == nil {
				res.Relations["outgoing"] = append(res.Relations["outgoing"], RelationResult{Type: rtype, Target: target})
			}
		}
	}

	// Query incoming relations
	queryIncoming := `
SELECT r.type, e.name
FROM relations r
JOIN entities e ON r.source_id = e.id
WHERE r.target_id = ?`

	res.Relations["incoming"] = make([]RelationResult, 0)
	rowsIn, err := d.DB.Query(queryIncoming, entityID)
	if err == nil {
		defer rowsIn.Close()
		for rowsIn.Next() {
			var rtype, source string
			if err := rowsIn.Scan(&rtype, &source); err == nil {
				res.Relations["incoming"] = append(res.Relations["incoming"], RelationResult{Type: rtype, Source: source})
			}
		}
	}

	// Reconstruct "Code" from chunks associated with this entity.
	queryChunks := `SELECT text FROM chunks WHERE entity_id = ? ORDER BY line_start ASC`
	rowsChunks, err := d.DB.Query(queryChunks, entityID)
	if err == nil {
		defer rowsChunks.Close()
		var codeBlocks []string
		for rowsChunks.Next() {
			var text string
			if err := rowsChunks.Scan(&text); err == nil {
				codeBlocks = append(codeBlocks, text)
			}
		}
		res.Code = strings.Join(codeBlocks, "\n\n")
	}

	return &res, nil
}
