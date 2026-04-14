use super::base::{Entity, Extractor, Relation};
use anyhow::Result;
use tree_sitter::{Parser, Query, QueryCursor};

pub struct GoExtractor {
    parser: Parser,
    content: String,
    entities: Vec<Entity>,
    relations: Vec<Relation>,
}

impl GoExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        // Since tree_sitter versions changed slightly, we rely on tree-sitter-go
        parser.set_language(&tree_sitter_go::language())?;
        Ok(Self {
            parser,
            content: String::new(),
            entities: Vec::new(),
            relations: Vec::new(),
        })
    }
}

impl Extractor for GoExtractor {
    fn parse(&mut self, content: &str) -> Result<()> {
        self.content = content.to_string();

        let tree = self
            .parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Go code"))?;

        let language = tree_sitter_go::language();

        // 1. Query for Entities
        let query_str = "
            (function_declaration name: (identifier) @func body: (block) @body)
            (type_spec name: (type_identifier) @type)
            (method_declaration name: (field_identifier) @method body: (block)? @body)
        ";
        let query = Query::new(&language, query_str)?;
        let mut cursor = QueryCursor::new();

        let call_query = Query::new(&language, "(call_expression function: (identifier) @call)")?;

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

                if capture_name == "func" || capture_name == "type" || capture_name == "method" {
                    ent_name = name;
                    ent_type = match capture_name {
                        "func" => "FUNCTION",
                        "type" => "TYPE",
                        "method" => "METHOD",
                        _ => "UNKNOWN",
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

                // 2. Extract Relations (CALLS) if there's a body
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
    fn test_go_extractor() {
        let code = r#"
package main

type Server struct {}

func (s *Server) Start() {
    InitDB()
    LogMessage()
}

func InitDB() {
}
        "#;

        let mut ext = GoExtractor::new().expect("Failed to init go extractor");
        ext.parse(code).expect("Failed to parse go code");

        let (entities, relations) = ext.extract().unwrap();

        let ent_names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(ent_names.contains(&"Server"));
        assert!(ent_names.contains(&"Start"));
        assert!(ent_names.contains(&"InitDB"));

        // Ensure CALLS relations exist
        let call_init = relations
            .iter()
            .find(|r| r.source == "Start" && r.target == "InitDB");
        let call_log = relations
            .iter()
            .find(|r| r.source == "Start" && r.target == "LogMessage");

        assert!(
            call_init.is_some(),
            "Relation Start -> CALLS -> InitDB missing"
        );
        assert!(
            call_log.is_some(),
            "Relation Start -> CALLS -> LogMessage missing"
        );
    }
}
