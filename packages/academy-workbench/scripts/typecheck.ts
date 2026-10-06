const compiler = new URL('../node_modules/typescript/bin/tsc', import.meta.url);
if (!await Bun.file(compiler).exists()) { console.error('NOT RUN: restore locked development dependencies with Bun in an authorized environment.'); process.exit(2); }
const result = Bun.spawn(['bun', fileURLToPath(compiler), '--project', 'tsconfig.json'], { stdout: 'inherit', stderr: 'inherit' });
process.exit(await result.exited);
import { fileURLToPath } from 'node:url';
