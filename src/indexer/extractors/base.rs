use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct Entity {
    pub name: String,
    pub entity_type: String, // FUNC, TYPE, METHOD
    pub qualified_name: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub source: String,
    pub target: String,
    pub relation_type: String, // CALLS, IMPLEMENTS
}

pub trait Extractor {
    fn parse(&mut self, content: &str) -> Result<()>;
    fn extract(&self) -> Result<(Vec<Entity>, Vec<Relation>)>;
}
