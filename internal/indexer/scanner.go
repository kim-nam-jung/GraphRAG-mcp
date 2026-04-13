package indexer

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"io/fs"
	"os"
	"path/filepath"

	"graphrag-mcp/internal/config"
	"graphrag-mcp/internal/storage"
)

type Scanner struct {
	db  *storage.Database
	cfg *config.IndexerConfig
}

func NewScanner(db *storage.Database, cfg *config.IndexerConfig) *Scanner {
	return &Scanner{
		db:  db,
		cfg: cfg,
	}
}

type fileState struct {
	mtime int64
	size  int64
	hash  string
}

// Scan processes a directory and detects changes for incremental indexing
func (s *Scanner) Scan(rootDir string) ([]string, []string, error) {
	var modifiedFiles []string
	var deletedFiles []string

	// 1. Load existing hashes from DB
	existing := make(map[string]fileState)
	rows, err := s.db.DB.Query("SELECT file_path, hash, mtime, size FROM file_hashes")
	if err == nil {
		defer rows.Close()
		for rows.Next() {
			var path, hash string
			var mtime, size int64
			if err := rows.Scan(&path, &hash, &mtime, &size); err == nil {
				existing[path] = fileState{mtime: mtime, size: size, hash: hash}
			}
		}
	}

	seen := make(map[string]bool)

	allowedExts := make(map[string]bool)
	for _, e := range s.cfg.Tier1 {
		if e != "" && e[0] != '.' { e = "." + e }
		allowedExts[e] = true 
	}
	for _, e := range s.cfg.Tier2 {
		if e != "" && e[0] != '.' { e = "." + e }
		allowedExts[e] = true 
	}
	for _, e := range s.cfg.Tier3 {
		if e != "" && e[0] != '.' { e = "." + e }
		allowedExts[e] = true 
	}

	// 2. Walk directory
	err = filepath.WalkDir(rootDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		
		// Skip excluded directories
		if d.IsDir() {
			base := filepath.Base(path)
			for _, excl := range s.cfg.ExcludeDirs {
				if base == excl {
					return filepath.SkipDir
				}
			}
			return nil
		}

		// Only process files with appropriate extensions (based on Tier1, 2, 3)
		ext := filepath.Ext(path)
		if len(allowedExts) > 0 && !allowedExts[ext] {
			return nil
		}
		
		info, err := d.Info()
		if err != nil {
			return nil
		}
		
		absPath, _ := filepath.Abs(path)
		seen[absPath] = true

		currentMTime := info.ModTime().Unix()
		currentSize := info.Size()

		if state, ok := existing[absPath]; ok {
			// Fast check
			if state.mtime == currentMTime && state.size == currentSize {
				return nil // Unchanged
			}

			// Size or mtime changed, calculate hash
			hash, err := calculateHash(absPath)
			if err != nil {
				return nil
			}

			if hash == state.hash {
				// Only mtime changed
				s.db.DB.Exec("UPDATE file_hashes SET mtime = ?, size = ? WHERE file_path = ?", currentMTime, currentSize, absPath)
				return nil
			}

			// Content really changed
			modifiedFiles = append(modifiedFiles, absPath)
			s.db.DB.Exec("UPDATE file_hashes SET hash = ?, mtime = ?, size = ? WHERE file_path = ?", hash, currentMTime, currentSize, absPath)
		} else {
			// New file
			hash, err := calculateHash(absPath)
			if err == nil {
				modifiedFiles = append(modifiedFiles, absPath)
				s.db.DB.Exec("INSERT INTO file_hashes (file_path, hash, mtime, size) VALUES (?, ?, ?, ?)", absPath, hash, currentMTime, currentSize)
			}
		}

		return nil
	})

	// 3. Find deleted files
	for path := range existing {
		if !seen[path] {
			deletedFiles = append(deletedFiles, path)
			// Trigger DB cascade deletion via relations and entities
			s.db.DB.Exec("DELETE FROM entities WHERE file_path = ?", path)
			s.db.DB.Exec("DELETE FROM file_hashes WHERE file_path = ?", path)
			// vec_chunks does not cascade from chunks, so we must manually delete vectors first
			s.db.DB.Exec("DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE file_path = ?)", path)
			// FTS and vector bindings updates are handled by sqlite triggers / cascades
			// (Note: chunks without ON DELETE CASCADE need manual deletion if not set null)
			s.db.DB.Exec("DELETE FROM chunks WHERE file_path = ?", path)
		}
	}

	return modifiedFiles, deletedFiles, err
}

func calculateHash(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()

	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}

// In a full implementation, the modifiedFiles would then be sent to the parser
// to extract entities, which are inserted:
// `INSERT INTO entities (name, qualified_name, type, file_path, line_start, line_end) ...`
// `INSERT INTO chunks (text, file_path, line_start, line_end, entity_id) ...`
