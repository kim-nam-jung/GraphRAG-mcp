#![allow(warnings)]
use anyhow::Result;
use graphrag_mcp::embedding::Tokenizer;
use graphrag_mcp::embedding::HarrierModel;
use graphrag_mcp::storage::Database;

#[test]
fn test_tokenizer_cl100k_base() -> Result<()> {
    let t = Tokenizer::new("")?;
    let text = "GraphRAG is heavily reliant on precise tokens.";
    let ids = t.encode(text, false)?;
    assert!(!ids.is_empty(), "Token array must not be empty");
    // cl100k_base should produce consistent token IDs
    let ids2 = t.encode(text, false)?;
    assert_eq!(ids, ids2, "Encoding must be deterministic");
    Ok(())
}

#[test]
fn test_harrier_fail_fast() {
    let bad_model = HarrierModel::new("invalid_path/model.onnx");
    assert!(
        bad_model.is_err(),
        "ONNX load failure must return Err, not a silent zero-vector mock"
    );
}

#[test]
fn test_database_schema() -> Result<()> {
    let db = Database::new(":memory:", false)?;

    // Verify core tables exist
    let entity_id = db.insert_entity("test.go", "main", "FUNCTION", "main.main")?;
    assert!(entity_id > 0);

    // Verify FTS works
    let results = db.search_fts("main", 5)?;
    assert!(!results.is_empty(), "FTS should find the inserted entity");

    // Verify chunk insert + line tracking
    let chunk_id = db.insert_chunk("func main() {}", "test.go", Some(1), Some(1), Some(entity_id))?;
    assert!(chunk_id > 0);

    // Verify get_entity with relations
    let detail = db.get_entity("main", "test.go")?;
    assert!(detail.is_some());

    Ok(())
}

#[test]
fn test_database_relations() -> Result<()> {
    let db = Database::new(":memory:", false)?;

    let id1 = db.insert_entity("test.go", "Foo", "FUNCTION", "pkg.Foo")?;
    let id2 = db.insert_entity("test.go", "Bar", "FUNCTION", "pkg.Bar")?;

    db.insert_relation(id1, id2, "CALLS", 1.0)?;

    let detail = db.get_entity("Foo", "test.go")?;
    assert!(detail.is_some());
    let detail = detail.unwrap();
    assert_eq!(detail.outgoing.len(), 1);
    assert_eq!(detail.outgoing[0].target_name, "Bar");

    Ok(())
}

#[test]
fn test_graph_neighbors() -> Result<()> {
    let db = Database::new(":memory:", false)?;

    let id_a = db.insert_entity("a.go", "A", "FUNCTION", "pkg.A")?;
    let id_b = db.insert_entity("b.go", "B", "FUNCTION", "pkg.B")?;
    let id_c = db.insert_entity("c.go", "C", "FUNCTION", "pkg.C")?;

    db.insert_relation(id_a, id_b, "CALLS", 1.0)?;
    db.insert_relation(id_b, id_c, "CALLS", 1.0)?;

    // depth=1 from A should find B
    let neighbors = db.graph_neighbors("A", 1, "outgoing")?;
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].name, "B");

    // depth=2 from A should find B and C
    let neighbors = db.graph_neighbors("A", 2, "outgoing")?;
    assert_eq!(neighbors.len(), 2);

    Ok(())
}
