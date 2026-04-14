import sqlite3
c = sqlite3.connect('/home/skawn1057/Development/GraphRAG-mcp/data/graph.db')
cursor = c.execute("SELECT id, name, qualified_name, file_path FROM entities WHERE name='new'")
for r in cursor.fetchall():
    print(r)
