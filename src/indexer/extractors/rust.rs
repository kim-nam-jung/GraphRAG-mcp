use anyhow::Result;
use tree_sitter::{Parser, Query, QueryCursor};
use super::base::{Entity, Relation, Extractor};

pub struct RustExtractor {
    parser: Parser,
    content: String,
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

impl RustExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::language())?;
        Ok(Self {
            parser,
            content: String::new(),
            entities: Vec::new(),
            relations: Vec::new(),
        })
    }
}

impl Extractor for RustExtractor {
    fn parse(&mut self, content: &str) -> Result<()> {
        self.content = content.to_string();
        
        let tree = self.parser.parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust code"))?;

        let language = tree_sitter_rust::language();
        let query_str = "
            (struct_item name: (type_identifier) @struct)
            (trait_item name: (type_identifier) @trait)
            (function_item name: (identifier) @func body: (block)? @body)
            (function_signature_item name: (identifier) @func)
        ";
        let query = Query::new(&language, query_str)?;
        let mut cursor = QueryCursor::new();
        
        let call_query = Query::new(&language, "(call_expression function: [ (identifier) @call (field_expression field: (field_identifier) @call) (scoped_identifier name: (identifier) @call) ])")?;

        let bindings = cursor.matches(&query, tree.root_node(), content.as_bytes());
        for m in bindings {
            let mut ent_name = String::new();
            let mut ent_type = "UNKNOWN";
            let mut body_node = None;
            let mut start_b = 0;
            let mut end_b = 0;

            for capture in m.captures {
                let name = capture.node.utf8_text(content.as_bytes())?.to_string();
                let capture_name = query.capture_names()[capture.index as usize];
                
                if capture_name == "struct" || capture_name == "trait" || capture_name == "func" {
                    ent_name = name;
                    ent_type = match capture_name {
                        "struct" => "STRUCT",
                        "trait" => "TRAIT",
                        _ => "FUNCTION",
                    };
                    let parent = capture.node.parent().unwrap_or(capture.node);
                    start_b = parent.start_byte();
                    end_b = parent.end_byte();
                } else if capture_name == "body" {
                    body_node = Some(capture.node);
                }
            }

            if !ent_name.is_empty() {
                self.entities.push(Entity {
                    name: ent_name.clone(),
                    entity_type: ent_type.to_string(),
                    qualified_name: ent_name.clone(),
                    start_byte: start_b,
                    end_byte: end_b,
                });

                if let Some(body) = body_node {
                    let mut call_cursor = QueryCursor::new();
                    let call_bindings = call_cursor.matches(&call_query, body, content.as_bytes());
                    for cm in call_bindings {
                        for c in cm.captures {
                            let target_call = c.node.utf8_text(content.as_bytes())?.to_string();
                            self.relations.push(Relation {
                                source: ent_name.clone(),
                                target: target_call,
                                relation_type: "CALLS".to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn extract(&self) -> Result<(Vec<Entity>, Vec<Relation>)> {
        Ok((self.entities.clone(), self.relations.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_extractor() {
        let mut extractor = RustExtractor::new().expect("Failed to create extractor");
        let code = r#"
            struct MyStruct { id: i32 }
            impl MyStruct {
                fn do_magic(&self) {}
            }
            fn main() {
                let x = MyStruct { id: 1 };
                x.do_magic();
            }
        "#;
        
        extractor.parse(code).expect("Extraction failed");
        let (entities, _) = extractor.extract().expect("Extraction failed");
        
        assert!(!entities.is_empty(), "Should extract entities");
        
        let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"MyStruct".to_string()));
        assert!(names.contains(&"do_magic".to_string()));
        assert!(names.contains(&"main".to_string()));
        
        for e in entities {
            assert!(e.end_byte > e.start_byte, "Byte span must be valid for {}", e.name);
        }
    }
}
