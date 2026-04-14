use anyhow::Result;
use tree_sitter::{Parser, Query, QueryCursor};
use super::base::{Entity, Relation, Extractor};

pub struct PhpExtractor {
    parser: Parser,
    content: String,
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

impl PhpExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_php::language_php())?;
        Ok(Self { parser, content: String::new(), entities: Vec::new(), relations: Vec::new() })
    }
}

impl Extractor for PhpExtractor {
    fn parse(&mut self, content: &str) -> Result<()> {
        self.content = content.to_string();
        let tree = self.parser.parse(content, None).ok_or_else(|| anyhow::anyhow!("Failed to parse PHP"))?;
        let language = tree_sitter_php::language_php();
        let query = Query::new(&language, "
            (class_declaration name: (name) @class)
            (interface_declaration name: (name) @interface)
            (method_declaration name: (name) @method)
        ")?;
        let mut cursor = QueryCursor::new();
        for m in cursor.matches(&query, tree.root_node(), content.as_bytes()) {
            for c in m.captures {
                let name = c.node.utf8_text(content.as_bytes())?.to_string();
                let cname = query.capture_names()[c.index as usize];
                let parent = c.node.parent().unwrap_or(c.node);
                self.entities.push(Entity { 
                    name: name.clone(), 
                    entity_type: cname.to_uppercase(), 
                    qualified_name: name,
                    start_byte: parent.start_byte(),
                    end_byte: parent.end_byte(),
                });
            }
        }
        Ok(())
    }
    fn extract(&self) -> Result<(Vec<Entity>, Vec<Relation>)> { Ok((self.entities.clone(), self.relations.clone())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_php_extractor() {
        let mut extractor = PhpExtractor::new().expect("Failed to create extractor");
        let code = "<?php\nclass MyClass {\n  function do_magic() {}\n}";
        
        extractor.parse(code).expect("Extraction failed");
        let (entities, _) = extractor.extract().expect("Extraction failed");
        
        assert!(!entities.is_empty(), "Should extract entities");
        
        let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"MyClass".to_string()));
        assert!(names.contains(&"do_magic".to_string()));
        
        for e in entities {
            assert!(e.end_byte > e.start_byte, "Byte span must be valid for {}", e.name);
        }
    }
}
