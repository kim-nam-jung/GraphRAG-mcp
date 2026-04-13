package indexer

import (
	"strings"

	"graphrag-mcp/internal/indexer/extractors"
)

type Chunk struct {
	Text      string
	LineStart int
	LineEnd   int
	// Pointer to the struct, nil if fallback chunk
	Entity *extractors.Entity
}

// ChunkFile creates semantic chunks based on structural entities.
// Fallback chunks will be created for lines not covered by any entity.
// MaxLines restricts the size of fallback chunks (and potentially entity chunks later).
func ChunkFile(content []byte, entities []extractors.Entity, maxLines int) []Chunk {
	lines := strings.Split(string(content), "\n")
	var chunks []Chunk

	// For MVP: We assume entities don't overlap in a way that breaks this simple model.
	// We create a chunk for each entity bounds.
	// We track covered lines to generate fallback chunks.

	covered := make([]bool, len(lines)+1) // 1-indexed

	for i := range entities {
		ent := &entities[i]
		start := ent.LineStart
		end := ent.LineEnd
		
		if start < 1 { start = 1 }
		// Ensure we don't exceed the file lines
		if end > len(lines) { end = len(lines) }
		if end < start { end = start }

		chunkText := strings.Join(lines[start-1:end], "\n")
		chunks = append(chunks, Chunk{
			Text:      chunkText,
			LineStart: start,
			LineEnd:   end,
			Entity:    ent,
		})

		for l := start; l <= end; l++ {
			covered[l] = true
		}
	}

	// Generate fallback chunks
	fallbackStart := -1
	for i := 1; i <= len(lines); i++ {
		if !covered[i] {
			if fallbackStart == -1 {
				fallbackStart = i
			}
		} else {
			if fallbackStart != -1 {
				// Close the fallback chunk
				fallbackEnd := i - 1
				chunks = append(chunks, createFallbackChunks(lines, fallbackStart, fallbackEnd, maxLines)...)
				fallbackStart = -1
			}
		}
	}

	if fallbackStart != -1 {
		chunks = append(chunks, createFallbackChunks(lines, fallbackStart, len(lines), maxLines)...)
	}

	return chunks
}

func createFallbackChunks(lines []string, start, end, maxLines int) []Chunk {
	var chunks []Chunk
	currStart := start

	for currStart <= end {
		currEnd := currStart + maxLines - 1
		if currEnd > end {
			currEnd = end
		}

		chunkText := strings.Join(lines[currStart-1:currEnd], "\n")
		// Trim empty fallback chunks
		if strings.TrimSpace(chunkText) != "" {
			chunks = append(chunks, Chunk{
				Text:      chunkText,
				LineStart: currStart,
				LineEnd:   currEnd,
				Entity:    nil,
			})
		}
		
		currStart = currEnd + 1
	}

	return chunks
}
