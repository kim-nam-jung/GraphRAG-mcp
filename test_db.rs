use rusqlite::Connection;

fn main() {
    let conn = Connection::open("./data/graph.db").unwrap();
    println!("DB Opened");
    let mut stmt = conn.prepare("SELECT name, type, qualified_name, community_id FROM entities").unwrap();
    let entity_iter = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let entity_type: String = row.get(1)?;
        let qualified_name: String = row.get(2)?;
        let community: Option<i32> = row.get(3)?;
        Ok((name, entity_type, qualified_name, community))
    }).unwrap();
    
    for e in entity_iter {
        match e {
            Ok(v) => {},
            Err(err) => { println!("Error row: {:?}", err); return; }
        }
    }
    println!("Entities success");

    let mut stmt = conn.prepare(
        "SELECT e1.name as source, e2.name as target, r.type as relation_type 
         FROM relations r
         JOIN entities e1 ON r.source_id = e1.id
         JOIN entities e2 ON r.target_id = e2.id"
    ).unwrap();
    let rel_iter = stmt.query_map([], |row| {
        let source: String = row.get(0)?;
        let target: String = row.get(1)?;
        let relation_type: String = row.get(2)?;
        Ok((source, target, relation_type))
    }).unwrap();
    for r in rel_iter {
        match r {
            Ok(v) => {},
            Err(err) => { println!("Error rel row: {:?}", err); return; }
        }
    }
    println!("Relations success");
}
