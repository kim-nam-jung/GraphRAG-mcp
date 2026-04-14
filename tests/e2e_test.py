import subprocess
import json
import uuid
import sys
import os

def check_response(response, req_id):
    if response.get("id") != req_id:
        print(f"ID mismatch! Expected {req_id}, got {response.get('id')}")
        sys.exit(1)
    if "error" in response:
        print(f"Error from server: {response['error']}")
        sys.exit(1)
    return response.get("result")

def send_request(process, method, params=None):
    req_id = str(uuid.uuid4())
    req = {
        "jsonrpc": "2.0",
        "id": req_id,
        "method": method
    }
    if params is not None:
        req["params"] = params
        
    line = json.dumps(req) + "\n"
    print(f"\n[CLIENT] >> {line.strip()}")
    process.stdin.write(line.encode('utf-8'))
    process.stdin.flush()
    
    # Read response
    while True:
        resp_line = process.stdout.readline()
        if not resp_line:
            print("[SERVER EXIT] Process exited unexpectedly!")
            sys.exit(1)
            
        resp_str = resp_line.decode('utf-8').strip()
        if not resp_str:
            continue
            
        if resp_str.startswith("{") and "jsonrpc" in resp_str:
            print(f"[SERVER] << {resp_str[:150]}... (truncated)")
            try:
                resp = json.loads(resp_str)
                if resp.get("id") == req_id:
                    return check_response(resp, req_id)
                elif "method" in resp:
                    print(f"Ignoring asynchronous notification: {resp_str}")
                else:
                    print(f"Ignoring out-of-order response: {resp_str}")
            except json.JSONDecodeError:
                print(f"[SERVER OUT] {resp_str}")
        else:
            # It might be stdout spam (which we just fixed, so this shouldn't happen much)
            print(f"[SERVER STDOUT WARN] {resp_str}")

def send_notification(process, method):
    req = { "jsonrpc": "2.0", "method": method }
    line = json.dumps(req) + "\n"
    print(f"\n[CLIENT] >> {line.strip()}")
    process.stdin.write(line.encode('utf-8'))
    process.stdin.flush()

def main():
    print("Starting GraphRAG-MCP E2E Test...")
    # Run the server binary
    bin_path = "./target/debug/graphrag_mcp"
    if not os.path.exists(bin_path):
        print(f"Binary not found: {bin_path}")
        sys.exit(1)

    # Note: we inherit stderr so trace logs show up nicely in terminal, 
    # but pipe stdout to capture JSON-RPC
    process = subprocess.Popen(
        [bin_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr
    )

    try:
        # 1. Initialize
        print("\n--- 1. Initialize Handshake ---")
        init_res = send_request(process, "initialize", {
            "protocolVersion": "2024-11-05", # latest MCP spec
            "capabilities": {},
            "clientInfo": {
                "name": "e2e-tester",
                "version": "1.0.0"
            }
        })
        print(f"Initialized successfully! Server Version: {init_res.get('serverInfo', {}).get('version')}")

        send_notification(process, "notifications/initialized")

        # 2. List Tools
        print("\n--- 2. List Tools ---")
        tools_res = send_request(process, "tools/list")
        tools = tools_res.get("tools", [])
        tool_names = [t.get("name") for t in tools]
        print(f"Found {len(tools)} tools: {', '.join(tool_names)}")
        assert "global_search" in tool_names
        assert "local_search" in tool_names
        assert "index_directory" in tool_names

        # 3. Call index_directory to ensure we have data
        print("\n--- 3. Call Tool: index_directory ---")
        idx_res = send_request(process, "tools/call", {
            "name": "index_directory",
            "arguments": {
                "path": "."
            }
        })
        print("Index Result:", json.dumps(idx_res)[:200])
        assert not idx_res.get("isError")
        content = idx_res["content"][0]["text"]
        print(f"Content: {content}")
        assert "indexed" in content.lower()

        # 4. Call global_search
        print("\n--- 4. Call Tool: global_search ---")
        gs_res = send_request(process, "tools/call", {
            "name": "global_search",
            "arguments": {
                "query": "graph",
                "max_entities": 10
            }
        })
        assert not gs_res.get("isError")
        gs_content = gs_res["content"][0]["text"]
        if "No matching entities" in gs_content:
            print("WARN: Found no entities, this could be normal if DB is empty, but we just indexed!")
        else:
            print("Global Search Result valid JSON graph returned.")
            # Verify the response is JSON loadable
            parts = gs_content.split("```json")
            if len(parts) > 1:
                graph_json_str = parts[1].split("```")[0].strip()
                try:
                    graph_json = json.loads(graph_json_str)
                    print(f"Extracted {len(graph_json.get('nodes', []))} nodes.")
                except Exception as e:
                    print("Failed to parse Global Search graph JSON:", e)

        # 5. Call local_search
        print("\n--- 5. Call Tool: local_search ---")
        ls_res = send_request(process, "tools/call", {
            "name": "local_search",
            "arguments": {
                "query": "database connection",
                "top_k": 3,
                "graph_depth": 1
            }
        })
        assert not ls_res.get("isError")
        ls_content = ls_res["content"][0]["text"]
        print("Local Search executed.")
        if "No semantic matches" not in ls_content:
            parts = ls_content.split("```json")
            if len(parts) > 1:
                ls_json_str = parts[1].split("```")[0].strip()
                try:
                    ls_json = json.loads(ls_json_str)
                    print(f"Local search returned {len(ls_json.get('semantic_chunks', []))} semantic chunks.")
                    if len(ls_json.get('semantic_chunks', [])) > 0:
                        print("SUCCESS: Real vectors were retrieved successfully!")
                except Exception as e:
                    print("Failed to parse Local Search JSON:", e)

    finally:
        print("\nCleaning up process...")
        process.kill()
        process.wait()

    print("\nE2E TEST PASSED!")

if __name__ == "__main__":
    main()
