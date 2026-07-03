import { spawn } from 'node:child_process';
import dotenv from 'dotenv';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
dotenv.config({ path: join(__dirname, '../.env') });

console.log('Starting local auth_api with env...');
const child = spawn('cargo', ['run', '--bin', 'auth_api'], {
  env: process.env,
  stdio: 'inherit'
});

child.on('exit', (code) => {
  console.log('Server exited with code:', code);
});
