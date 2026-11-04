import {configureTrace,trace} from './guard.ts';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const [base,scratch]=process.argv.slice(2);
const file=resolve(scratch,'effects.ndjson');writeFileSync(file,readFileSync(resolve(base,'scratch/effects.ndjson')));configureTrace(file);
try{await fetch('https://never-dispatched.invalid/');}catch{}
process.stdout.write(readFileSync(resolve(base,'stdout.bin')));
