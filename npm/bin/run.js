#!/usr/bin/env node

const os = require('os');
const path = require('path');
const fs = require('fs');
const { spawn } = require('child_process');

// Determine correct binary name based on OS and architecture
function getBinaryName() {
    const platform = os.platform();
    const arch = os.arch();

    if (platform === 'win32') return 'graphrag-mcp.exe';
    if (platform === 'darwin') {
        if (arch === 'arm64') return 'graphrag-mcp-macos-arm64';
        return 'graphrag-mcp-macos-x64';
    }
    if (platform === 'linux') {
        return 'graphrag-mcp-linux-x64';
    }
    
    throw new Error(`Unsupported platform: ${platform} ${arch}`);
}

async function main() {
    try {
        const binName = getBinaryName();
        // In a real publication, the binary would be downloaded from GitHub releases
        // if not present, and cached in ~/.graphrag-mcp/bin/
        
        // For development scaffolding, we'll assume it's in the same or target dir
        const localDevPath = path.join(__dirname, '..', '..', 'target', 'release', binName.includes('.exe') ? 'graphrag_mcp.exe' : 'graphrag_mcp');
        const binPath = fs.existsSync(localDevPath) ? localDevPath : binName; // Fallback to searching PATH
        
        console.error(`[GraphRAG-MCP Launcher] Starting platform binary: ${binPath}`);

        const child = spawn(binPath, process.argv.slice(2), {
            stdio: 'inherit',
            env: process.env
        });

        child.on('error', (err) => {
            console.error('[GraphRAG-MCP Launcher] Failed to start binary. Please make sure the precompiled rust engine is available.', err);
            process.exit(1);
        });

        child.on('close', (code) => {
            process.exit(code || 0);
        });

    } catch (err) {
        console.error('[GraphRAG-MCP Launcher] Initialization error:', err.message);
        process.exit(1);
    }
}

main();
