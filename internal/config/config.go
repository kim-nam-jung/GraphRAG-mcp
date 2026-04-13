package config

import (
	"os"

	"gopkg.in/yaml.v3"
)

type ServerConfig struct {
	Transport string `yaml:"transport"`
}

type IndexerConfig struct {
	Tier1         []string `yaml:"tier1"`
	Tier2         []string `yaml:"tier2"`
	Tier3         []string `yaml:"tier3"`
	ExcludeDirs   []string `yaml:"exclude_dirs"`
	ExcludeFiles  []string `yaml:"exclude_files"`
	ChunkMaxLines int      `yaml:"chunk_max_lines"`
}

type EmbeddingConfig struct {
	ModelPath        string `yaml:"model_path"`
	TokenizerPath    string `yaml:"tokenizer_path"`
	Quantization     string `yaml:"quantization"`
	Dimension        int    `yaml:"dimension"`
	MaxTokens        int    `yaml:"max_tokens"`
	QueryInstruction string `yaml:"query_instruction"`
}

type StorageConfig struct {
	DBPath  string `yaml:"db_path"`
	WALMode bool   `yaml:"wal_mode"`
}

type GraphConfig struct {
	LeidenResolution float64 `yaml:"leiden_resolution"`
	MinCommunitySize int     `yaml:"min_community_size"`
}

type SearchConfig struct {
	AutoReindex        bool `yaml:"auto_reindex"`
	ReindexCooldownSec int  `yaml:"reindex_cooldown_sec"`
}

type Config struct {
	Server    ServerConfig    `yaml:"server"`
	Indexer   IndexerConfig   `yaml:"indexer"`
	Embedding EmbeddingConfig `yaml:"embedding"`
	Storage   StorageConfig   `yaml:"storage"`
	Graph     GraphConfig     `yaml:"graph"`
	Search    SearchConfig    `yaml:"search"`
}

// LoadConfig reads and parses the YAML configuration file
func LoadConfig(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var cfg Config
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}

	return &cfg, nil
}
