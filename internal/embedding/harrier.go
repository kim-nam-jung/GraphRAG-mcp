package embedding

import (
	"fmt"
	"math"
	"os"
	"runtime"

	ort "github.com/yalue/onnxruntime_go"
)

type HarrierModel struct {
	session *ort.DynamicAdvancedSession
	dim     int
}

func NewHarrierModel(modelPath string) (*HarrierModel, error) {
	if !ort.IsInitialized() {
		// Dynamically assign ONNX runtime library based on OS
		var sharedLibPath string
		switch runtime.GOOS {
		case "windows":
			sharedLibPath = "onnxruntime.dll"
		case "darwin":
			sharedLibPath = "libonnxruntime.dylib"
		default:
			sharedLibPath = "/usr/lib/libonnxruntime.so"
		}
		
		ort.SetSharedLibraryPath(sharedLibPath)
		ort.InitializeEnvironment()
	}

	// Harrier text embeddings generally take input_ids and attention_mask
	inputNames := []string{"input_ids", "attention_mask"}
	outputNames := []string{"last_hidden_state"}

	options, err := ort.NewSessionOptions()
	if err != nil {
		fmt.Fprintf(os.Stderr, "[HarrierModel] Warning: session options creation failed: %v\n", err)
		return &HarrierModel{dim: 640}, nil
	}
	defer options.Destroy()

	session, err := ort.NewDynamicAdvancedSession(modelPath, inputNames, outputNames, options)
	if err != nil {
		fmt.Fprintf(os.Stderr, "[HarrierModel] Warning: ONNX model not loaded, returning mock engine (err: %v)\n", err)
		return &HarrierModel{dim: 640}, nil
	}

	fmt.Fprintf(os.Stderr, "[HarrierModel] ONNX execution session loaded successfully\n")
	return &HarrierModel{session: session, dim: 640}, nil
}

// Embed generates embeddings from text, applying the instruction prefix if query
func (h *HarrierModel) Embed(text string, isQuery bool, instruction string, tokenizer *Tokenizer) ([]float32, error) {
	if isQuery && instruction != "" {
		text = instruction + text
	}

	tokens, err := tokenizer.Encode(text, true)
	if err != nil {
		return nil, err
	}

	seqLen := int64(len(tokens))
	if seqLen == 0 {
		return make([]float32, h.dim), nil
	}

	// If session isn't loaded (standalone test mode), return mocked zeros
	if h.session == nil {
		return make([]float32, h.dim), nil
	}

	inputShape := ort.NewShape(1, seqLen)
	
	inputData := make([]int64, seqLen)
	maskData := make([]int64, seqLen)
	for i, t := range tokens {
		inputData[i] = int64(t)
		maskData[i] = 1 // Attention mask ignores nothing in simple sentence processing
	}

	tensorA, err := ort.NewTensor(inputShape, inputData)
	if err != nil { return nil, err }
	defer tensorA.Destroy()

	tensorB, err := ort.NewTensor(inputShape, maskData)
	if err != nil { return nil, err }
	defer tensorB.Destroy()

	outShape := ort.NewShape(1, seqLen, int64(h.dim))
	outData := make([]float32, seqLen*int64(h.dim))
	tensorOut, err := ort.NewTensor(outShape, outData)
	if err != nil { return nil, err }
	defer tensorOut.Destroy()

	err = h.session.Run(
		[]ort.ArbitraryTensor{tensorA, tensorB},
		[]ort.ArbitraryTensor{tensorOut},
	)
	if err != nil {
		return nil, fmt.Errorf("ONNX inference failed: %w", err)
	}

	// Last-token pooling
	emb := make([]float32, h.dim)
	lastIdx := seqLen - 1
	for d := 0; d < h.dim; d++ {
		emb[d] = outData[lastIdx*int64(h.dim)+int64(d)]
	}

	// Apply L2 normalization (often required for MTEB standard vector queries)
	var sumSq float64
	for _, v := range emb {
		sumSq += float64(v * v)
	}
	norm := float32(math.Sqrt(sumSq))
	if norm > 0 {
		for i := range emb {
			emb[i] /= norm
		}
	}

	return emb, nil
}

func (h *HarrierModel) Close() {
	if h.session != nil {
		h.session.Destroy()
	}
}
