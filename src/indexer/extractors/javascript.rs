use anyhow::Result;
use tree_sitter::{Parser, Query, QueryCursor};
use super::base::{Entity, Relation, Extractor};

pub struct JsExtractor {
    parser: Parser,
    content: String,
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

impl JsExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_javascript::language())?;
        Ok(Self {
            parser,
            content: String::new(),
            entities: Vec::new(),
            relations: Vec::new(),
        })
    }
}

impl Extractor for JsExtractor {
    fn parse(&mut self, content: &str) -> Result<()> {
        self.content = content.to_string();
        
        let tree = self.parser.parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse JS code"))?;

        let language = tree_sitter_javascript::language();
        let query_str = "
            (class_declaration name: (identifier) @class body: (class_body)? @body)
            (function_declaration name: (identifier) @func body: (statement_block)? @body)
            (method_definition name: (property_identifier) @method body: (statement_block)? @body)
            (arrow_function) @arrow
        ";
        let query = Query::new(&language, query_str)?;
        let mut cursor = QueryCursor::new();
        
        // Find calls specifically looking at proper identifiers or member expressions
        let call_query = Query::new(&language, "(call_expression function: [ (identifier) @call (member_expression property: (property_identifier) @call) ])")?;

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
                
                if capture_name == "class" || capture_name == "func" || capture_name == "method" || capture_name == "arrow" {
                    ent_name = name;
                    ent_type = match capture_name {
                        "class" => "CLASS",
                        "method" => "METHOD",
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
    fn test_js_extractor() {
        let code = r#"
            class UserManager {
                login() {
                    this.authenticate();
                    validateToken();
                }
            }

            function validateToken() {
                console.log("valid");
            }
        "#;
        
        let mut ext = JsExtractor::new().expect("Failed to create extractor");
        ext.parse(code).expect("Failed to parse JS code");
        
        let (entities, relations) = ext.extract().unwrap();
        
        // Ensure Entities are caught (class, method, func)
        let ent_names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(ent_names.contains(&"UserManager"));
        assert!(ent_names.contains(&"login"));
        assert!(ent_names.contains(&"validateToken"));
        
        // Ensure CALLS relations are mapped correctly
        let validate_call = relations.iter().find(|r| r.source == "login" && r.target == "validateToken");
        assert!(validate_call.is_some(), "Relation login -> CALLS -> validateToken should exist");
    }
}
