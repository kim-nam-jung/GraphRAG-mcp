use anyhow::Result;
use tree_sitter::{Parser, Query, QueryCursor};
use super::base::{Entity, Relation, Extractor};

pub struct CSharpExtractor {
    parser: Parser,
    content: String,
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

impl CSharpExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_c_sharp::language())?;
        Ok(Self { parser, content: String::new(), entities: Vec::new(), relations: Vec::new() })
    }
}

impl Extractor for CSharpExtractor {
    fn parse(&mut self, content: &str) -> Result<()> {
        self.content = content.to_string();
        let tree = self.parser.parse(content, None).ok_or_else(|| anyhow::anyhow!("Failed to parse C#"))?;
        let language = tree_sitter_c_sharp::language();
        let query = Query::new(&language, "
            (class_declaration name: (identifier) @class)
            (interface_declaration name: (identifier) @interface)
            (method_declaration name: (identifier) @method body: (block)? @body)
        ")?;
        let call_query = Query::new(&language, "
            (invocation_expression function: [ (identifier) @call (member_access_expression name: (identifier) @call) ])
        ")?;

        let mut cursor = QueryCursor::new();
        let bindings = cursor.matches(&query, tree.root_node(), content.as_bytes());
        for m in bindings {
            let mut ent_name = String::new();
            let mut ent_type = "UNKNOWN";
            let mut body_node = None;
            let mut start_b = 0;
            let mut end_b = 0;

            for c in m.captures {
                let name = c.node.utf8_text(content.as_bytes())?.to_string();
                let cname = query.capture_names()[c.index as usize];
                
                if cname == "class" || cname == "interface" || cname == "method" {
                    ent_name = name;
                    ent_type = match cname {
                        "class" => "CLASS",
                        "interface" => "INTERFACE",
                        _ => "METHOD",
                    };
                    let parent = c.node.parent().unwrap_or(c.node);
                    start_b = parent.start_byte();
                    end_b = parent.end_byte();
                } else if cname == "body" {
                    body_node = Some(c.node);
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
    fn extract(&self) -> Result<(Vec<Entity>, Vec<Relation>)> { Ok((self.entities.clone(), self.relations.clone())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_sharp_extractor() {
        let mut extractor = CSharpExtractor::new().expect("Failed to create extractor");
        let code = r#"
            public class MyClass {
                public void DoMagic() {}
            }
        "#;
        
        extractor.parse(code).expect("Extraction failed");
        let (entities, _) = extractor.extract().expect("Extraction failed");
        
        assert!(!entities.is_empty(), "Should extract entities");
        
        let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"MyClass".to_string()));
        assert!(names.contains(&"DoMagic".to_string()));
        
        for e in entities {
            assert!(e.end_byte > e.start_byte, "Byte span must be valid for {}", e.name);
        }
    }
}
