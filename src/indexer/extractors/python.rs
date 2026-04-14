use super::base::{Entity, Extractor, Relation};
use anyhow::Result;
use tree_sitter::{Parser, Query, QueryCursor};

pub struct PyExtractor {
    parser: Parser,
    content: String,
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

impl PyExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_python::language())?;
        Ok(Self {
            parser,
            content: String::new(),
            entities: Vec::new(),
            relations: Vec::new(),
        })
    }
}

impl Extractor for PyExtractor {
    fn parse(&mut self, content: &str) -> Result<()> {
        self.content = content.to_string();

        let tree = self
            .parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python code"))?;

        let language = tree_sitter_python::language();
        let query_str = "
            (class_definition name: (identifier) @class body: (block)? @body)
            (function_definition name: (identifier) @func body: (block)? @body)
        ";
        let query = Query::new(&language, query_str)?;
        let mut cursor = QueryCursor::new();

        let call_query_str = "
            (call function: (identifier) @call)
            (call function: (attribute attribute: (identifier) @call))
        ";
        let call_query = Query::new(&language, call_query_str)?;

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

                if capture_name == "class" || capture_name == "func" {
                    ent_name = name;
                    ent_type = if capture_name == "class" {
                        "CLASS"
                    } else {
                        "FUNCTION"
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
    fn test_python_extractor() {
        let code = r#"
class AuthHandler:
    def verify_request(self):
        check_signature()
        self.load_user()

def check_signature():
    print("Checked")
        "#;

        let mut ext = PyExtractor::new().expect("Failed to init py extractor");
        ext.parse(code).expect("Failed to parse PY code");

        let (entities, relations) = ext.extract().unwrap();

        let ent_names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(ent_names.contains(&"AuthHandler"));
        assert!(ent_names.contains(&"verify_request"));
        assert!(ent_names.contains(&"check_signature"));

        // Ensure CALLS relations exist
        let call_check = relations
            .iter()
            .find(|r| r.source == "verify_request" && r.target == "check_signature");
        let call_load = relations
            .iter()
            .find(|r| r.source == "verify_request" && r.target == "load_user");

        assert!(
            call_check.is_some(),
            "Relation verify_request -> CALLS -> check_signature missing"
        );
        assert!(
            call_load.is_some(),
            "Relation verify_request -> CALLS -> load_user missing"
        );
    }
}
