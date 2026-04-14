import urllib.request, json
import time
for crate in ['tree-sitter-php', 'tree-sitter-scala', 'tree-sitter-swift', 'tree-sitter-kotlin', 'devgen-tree-sitter-swift']:
    try:
        url = f'https://crates.io/api/v1/crates/{crate}'
        req = urllib.request.Request(url, headers={'User-Agent': 'Antigravity/1.0'})
        data = json.loads(urllib.request.urlopen(req).read())
        time.sleep(0.5)
        for v in data['versions'][:15]: # check latest 15 versions
            deps_url = f'https://crates.io/api/v1/crates/{crate}/{v["num"]}/dependencies'
            req2 = urllib.request.Request(deps_url, headers={'User-Agent': 'Antigravity/1.0'})
            try:
                deps = json.loads(urllib.request.urlopen(req2).read())['dependencies']
                time.sleep(0.2)
                ts_dep = next((d for d in deps if d['crate_id'] == 'tree-sitter'), None)
                if ts_dep:
                    print(f'{crate} version {v["num"]} needs tree-sitter {ts_dep["req"]}')
            except:
                pass
    except Exception as e:
        print(f"Failed {crate}: {e}")
