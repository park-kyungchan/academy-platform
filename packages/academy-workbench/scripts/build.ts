import { mkdir, copyFile } from 'node:fs/promises';
const result = await Bun.build({ entrypoints: ['src/ui/main.ts'], outdir: 'dist', naming: 'app.js', target: 'browser', minify: true });
if (!result.success) { console.error(result.logs); process.exit(1); }
await mkdir('dist', { recursive: true });
await Promise.all(['index.html', 'styles.css'].map(f => copyFile(`ui/${f}`, `dist/${f}`)));
await Bun.write('dist/styles.css', `${await Bun.file('ui/styles.css').text()}\n${await Bun.file('ui/curriculum.css').text()}`);
console.log('PASS: local UI built to dist (no external assets)');
