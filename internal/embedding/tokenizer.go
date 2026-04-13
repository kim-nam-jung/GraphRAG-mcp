package embedding

import (
	"fmt"
	"os"

	"github.com/pkoukk/tiktoken-go"
)

type Tokenizer struct {
	tkm *tiktoken.Tiktoken
}

func NewTokenizer(tokenizerPath string) (*Tokenizer, error) {
	// Our Harrier model requires standard cl100k_base embedding (equivalent to tokenizer.json in Nomic)
	encoding := "cl100k_base"
	
	tk, err := tiktoken.GetEncoding(encoding)
	if err != nil {
		return nil, fmt.Errorf("failed to load %s tokenizer: %w", encoding, err)
	}

	fmt.Fprintf(os.Stderr, "[Tokenizer] Loaded pure-Go Tiktoken tokenizer for %s\n", encoding)
	return &Tokenizer{tkm: tk}, nil
}

// Encode converts text into token IDs
func (t *Tokenizer) Encode(text string, addSpecialTokens bool) ([]uint32, error) {
	var ids []int
	if addSpecialTokens {
		// allow all special tokens during encoding
		ids = t.tkm.Encode(text, []string{"all"}, []string{})
	} else {
		// block special tokens
		ids = t.tkm.Encode(text, nil, nil)
	}

	// Downcast to uint32 to match Harrier ONNX tensor types
	uintIds := make([]uint32, len(ids))
	for i, id := range ids {
		uintIds[i] = uint32(id)
	}
	
	return uintIds, nil
}

func (t *Tokenizer) Close() {
	// Note: tiktoken-go holds regex and maps in GC managed memory structs.
	// We don't need manual freeing like CGO requires.
	t.tkm = nil
}
