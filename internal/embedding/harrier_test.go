package embedding

import (
	"math"
	"testing"
)

func TestHarrierModel_Mock(t *testing.T) {
	// A missing path will instantiate a mock model
	model, err := NewHarrierModel("non_existent_folder/model.onnx")
	if err != nil {
		t.Fatalf("unexpected error creating mock harrier model: %v", err)
	}
	defer model.Close()

	if model.dim != 640 {
		t.Fatalf("expected mocked dimension 640, got %d", model.dim)
	}

	// We pass a nil Tokenizer for a quick check. Wait, embed needs tokenizer.
	tokenizer, err := NewTokenizer("")
	if err != nil {
		t.Skip("skipping embedding test because tokenizer dict is not available")
	}

	emb, err := model.Embed("hello world", true, "Test query: ", tokenizer)
	if err != nil {
		t.Fatalf("embedding failed: %v", err)
	}

	if len(emb) != 640 {
		t.Fatalf("expected 640 dim embedding, got %d", len(emb))
	}

	// Check if elements are mocked correctly (should be zeros)
	var sum float32
	for _, v := range emb {
		sum += float32(math.Abs(float64(v)))
	}
	if sum != 0 {
		t.Fatalf("expected mock embedding to be zero array, got sum %f", sum)
	}
}
