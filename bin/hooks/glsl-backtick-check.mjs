#!/usr/bin/env node
// Claude Code PostToolUse hook (Write|Edit) - GLSL backtick guard.
//
// A backtick anywhere inside a /* glsl */ template literal - even in a GLSL
// comment - terminates the JS template literal early and breaks the whole
// build (the recurring foot-gun). This hook scans the written file and BLOCKS
// with feedback so the agent removes the stray backtick in the same turn,
// instead of discovering it at build time.
//
// Detection: valid GLSL can never contain a backtick, so inside a marked
// literal the only legitimate backticks are (a) the closing one - identified
// as a backtick followed by ; , ) or . (statement/argument end) - and
// (b) backticks inside ${...} interpolation, which are skipped. Anything
// else between the opening backtick and the terminator is a stray.
// Bias is toward false negatives (skip when a terminator can't be located)
// so the hook never blocks on a file it can't confidently parse.
//
// stdin:  hook JSON {tool_input:{file_path}, ...}
// stdout: {"decision":"block", ...} on violation; silent exit 0 otherwise.
import { readFileSync } from 'node:fs';

let input;
try { input = JSON.parse(readFileSync(0, 'utf8')); } catch { process.exit(0); }
const file = input?.tool_input?.file_path ?? input?.tool_response?.filePath;
if (!file || !/\.(m?[tj]s|tsx|jsx)$/.test(file)) process.exit(0);

let src;
try { src = readFileSync(file, 'utf8'); } catch { process.exit(0); }

const MARK = /\/\*\s*glsl\s*\*\//g;
const badLines = new Set();

let m;
while ((m = MARK.exec(src)) !== null) {
  // Skip marker text that merely appears in a // comment (prose about the convention).
  const lineStart = src.lastIndexOf('\n', m.index) + 1;
  if (src.slice(lineStart, m.index).includes('//')) continue;
  // A real tag directly precedes its literal: /* glsl */ `...` (whitespace only between).
  const open = src.indexOf('`', m.index + m[0].length);
  if (open === -1) continue;
  if (!/^\s*$/.test(src.slice(m.index + m[0].length, open))) continue;
  const strays = [];
  let end = -1;
  let i = open + 1;
  while (i < src.length) {
    const c = src[i];
    if (c === '$' && src[i + 1] === '{') { // skip ${...} interpolation
      let depth = 1;
      i += 2;
      while (i < src.length && depth > 0) {
        if (src[i] === '{') depth++;
        else if (src[i] === '}') depth--;
        i++;
      }
      continue;
    }
    if (c === '`') {
      if (/^\s*[;,).]/.test(src.slice(i + 1, i + 8))) { end = i; break; }
      strays.push(i);
    }
    i++;
  }
  if (end === -1) continue; // no confident terminator - do not guess
  for (const s of strays) badLines.add(src.slice(0, s).split('\n').length);
  MARK.lastIndex = end + 1;
}

if (badLines.size === 0) process.exit(0);

const lines = [...badLines].sort((a, b) => a - b).join(', ');
console.log(JSON.stringify({
  decision: 'block',
  reason:
    `Stray backtick inside a /* glsl */ template literal in ${file} at line(s) ${lines}. ` +
    'A backtick anywhere in GLSL - even in a comment - closes the JS template literal and breaks the whole build. ' +
    'Edit the file now to remove the backtick(s); rewrite comments without markdown code spans (quote names instead).',
  systemMessage: `GLSL backtick guard: stray backtick(s) in ${file} line(s) ${lines}`,
}));
