#!/usr/bin/env node
/**
 * Cross-platform sidecar build script
 * Calls the appropriate platform-specific script (bash or PowerShell)
 */

import { spawn } from 'child_process';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const isWindows = process.platform === 'win32';

// Get target from command line or environment
const target = process.argv[2] || process.env.SIDECAR_TARGET || '';

let command, args;

if (isWindows) {
    command = 'powershell';
    args = ['-ExecutionPolicy', 'Bypass', '-File', join(__dirname, 'build-sidecar.ps1')];
    if (target) args.push('-Target', target);
} else {
    command = 'bash';
    args = [join(__dirname, 'build-sidecar.sh')];
    if (target) args.push(target);
}

console.log(`Running: ${command} ${args.join(' ')}`);

const child = spawn(command, args, {
    stdio: 'inherit',
    cwd: __dirname
});

child.on('close', (code) => {
    process.exit(code || 0);
});

child.on('error', (err) => {
    console.error('Failed to start subprocess:', err);
    process.exit(1);
});
