package embedding

import (
	"testing"
)

func TestTokenizer(t *testing.T) {
	tokenizer, err := NewTokenizer("cl100k_base")
	if err != nil {
		t.Fatalf("failed to initialize tokenizer: %v", err)
	}
	defer tokenizer.Close()

	if tokenizer.tkm == nil {
		t.Fatal("expected tiktoken instance to be initialized, got nil")
	}

	text := "Hello World, this is a test!"
	
	// Test Encode WITHOUT special tokens
	idsNoSpecial, err := tokenizer.Encode(text, false)
	if err != nil {
		t.Fatalf("failed to encode text without special tokens: %v", err)
	}
	if len(idsNoSpecial) == 0 {
		t.Fatal("expected non-empty tokens, got empty")
	}

	// Test Encode WITH special tokens
	textWithSpecial := "<|endoftext|>"
	idsSpecial, err := tokenizer.Encode(textWithSpecial, true)
	if err != nil {
		t.Fatalf("failed to encode text with special tokens: %v", err)
	}
	if len(idsSpecial) == 0 {
		t.Fatal("expected non-empty tokens when string contains special tokens")
	}

	// Make sure they are typed properly as uint32 for ONNX compatibility
	var _ uint32 = idsNoSpecial[0]
}
