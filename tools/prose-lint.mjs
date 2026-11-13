#!/usr/bin/env node
// Flags the breaks of .claude/rules/writing.md that a program can catch, such as a banned word
// or a link to a heading that doesn't exist, in Markdown and in the comments in code.
//
//   node tools/prose-lint.mjs <file>...      Lint whole files. Exits 1 if anything is found.
//   node tools/prose-lint.mjs --hook         Lint only what a Write or Edit just wrote, reading the
//                                              PostToolUse payload on stdin. Findings reach the agent
//                                              as context, and the edit itself is never blocked.
//
// The word list is the table between the prose-lint markers in .claude/rules/writing.md, so the guide and
// the lint can't disagree about what gets flagged. Judgment calls belong to the doc-reviewer agent.

import { existsSync, readFileSync } from 'node:fs'
import { dirname, isAbsolute, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const GUIDE = resolve(ROOT, '.claude/rules/writing.md')
const TERMS_START = '<!-- prose-lint: terms -->'
const TERMS_END = '<!-- prose-lint: end -->'
const HOOK_MAX = 25
const BANNED_HEADINGS = new Set(['overview', 'introduction', 'intro', 'summary', 'conclusion', 'conclusions'])
// Plain entries in the table also catch these endings: `robust` → `robustness`, `leverage` → `leveraged`.
const INFLECTIONS = '(?:s|es|d|ed|ing|ly|ness|al|ful)?'
const CODE_FILE = /\.(?:[cm]?[jt]sx?|cpp|cc|h|hpp|rs|json)$/i
const HASH_FILE = /\.toml$/i
const SKIPPED = /(?:^|\/)node_modules\/|^\.claude\/worktrees\//
// A comment that tells the code's history goes stale, and git keeps the history anyway, so
// writing.md's Code comments rules it out. The pattern comes from Afterfund's prose-lint
// (AfterfundTech/afterfund, .claude/hooks/prose-lint.mjs), without its bare "now" and "used to".
// In our probes' comments, the five such hits we checked meant the moment the code runs, a purpose
// ("used to tell"), or a variable named NOW.
const HISTORY = /(?<![.\w])(?:previously|no longer|used to be|for now|we changed|was changed)(?![\w(])/gi

function loadTerms() {
  const guide = readFileSync(GUIDE, 'utf8').replace(/\r\n/g, '\n')
  const start = guide.indexOf(TERMS_START)
  const end = guide.indexOf(TERMS_END)
  if (start < 0 || end < start) {
    throw new Error(`.claude/rules/writing.md has no table between ${TERMS_START} and ${TERMS_END}`)
  }
  const terms = []
  for (const row of guide.slice(start, end).split('\n')) {
    if (!row.startsWith('|')) continue
    // A regex entry escapes its pipes as \| so the table still parses.
    const [cut = '', why = '', instead = ''] = row
      .split(/(?<!\\)\|/)
      .slice(1, -1)
      .map((cell) => cell.trim())
    for (const [, raw] of cut.matchAll(/`([^`]+)`/g)) {
      const source = raw.replace(/\\\|/g, '|')
      terms.push({ why, instead, re: toRegex(source) })
    }
  }
  return terms
}

function toRegex(term) {
  if (term.length > 2 && term.startsWith('/') && term.endsWith('/')) {
    return new RegExp(term.slice(1, -1), 'gi')
  }
  const body = term
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    .replace(/'/g, "['’]")
    .replace(/\s+/g, '\\s+')
  return new RegExp(`(?<![\\w'’])${body}${INFLECTIONS}(?![\\w'’])`, 'gi')
}

// Within a line, `code` keeps only code spans masked, and `prose` also masks link targets, URLs,
// and quoted text, because a word in quotes is being mentioned rather than used.
function maskLine(line) {
  const code = line.replace(/(`+).*?\1/g, (m) => ' '.repeat(m.length))
  const prose = code
    .replace(/\]\([^)]*\)/g, (m) => ']' + ' '.repeat(m.length - 1))
    .replace(/<https?:[^>]*>|https?:\/\/\S+/g, (m) => ' '.repeat(m.length))
    .replace(/"[^"]*"|“[^”]*”/g, (m) => ' '.repeat(m.length))
  return { code, prose }
}

// Returns the Markdown lines the rules apply to, each tagged with the paragraph ("unit") it
// belongs to. Front matter, code blocks, HTML comments, and the terms table are left out.
function scan(text) {
  const lines = text.split('\n')
  const out = []
  let unit = 0
  let fence = null
  let inComment = false
  let inTerms = false
  let prevBlank = true
  let prevKind = 'none'
  let i = 0
  if (lines[0] === '---') {
    const close = lines.indexOf('---', 1)
    if (close > 0) i = close + 1
  }
  for (; i < lines.length; i++) {
    const line = lines[i]
    const trimmed = line.trim()
    if (fence) {
      if (trimmed.startsWith(fence)) fence = null
      unit++
      continue
    }
    const fenceOpen = trimmed.match(/^(```+|~~~+)/)
    if (fenceOpen) {
      fence = fenceOpen[1]
      prevKind = 'code'
      prevBlank = false
      unit++
      continue
    }
    if (inTerms || trimmed.startsWith(TERMS_START)) {
      inTerms = !trimmed.startsWith(TERMS_END)
      unit++
      continue
    }
    if (inComment || trimmed.startsWith('<!--')) {
      inComment = !trimmed.includes('-->')
      unit++
      continue
    }
    if (!trimmed) {
      prevBlank = true
      unit++
      continue
    }
    const indent = line.match(/^[ \t]*/)[0].replace(/\t/g, '    ').length
    const listItem = /^\s*(?:[-*+]|\d+[.)])\s+/.test(line)
    // Four spaces after a blank line is a code block, unless it continues a list item.
    if (indent >= 4 && !listItem && (prevKind === 'code' || (prevBlank && prevKind !== 'list'))) {
      prevKind = 'code'
      prevBlank = false
      unit++
      continue
    }
    const heading = /^#{1,6}\s/.test(trimmed)
    const table = trimmed.startsWith('|')
    if (heading || table || listItem || prevBlank || prevKind === 'heading' || prevKind === 'table') unit++
    if (heading) prevKind = 'heading'
    else if (table) prevKind = 'table'
    else if (listItem) prevKind = 'list'
    else if (!(prevKind === 'list' && indent >= 2)) prevKind = 'para'
    prevBlank = false
    out.push({ n: i + 1, ...maskLine(line), heading, unit })
  }
  return out
}

// Returns each line of comment text in a C-like file: JavaScript, TypeScript, C++, Rust, or JSON
// with comments, such as a tsconfig. It skips strings, so a `//` inside one isn't read as a
// comment. Three things fool it: a regex literal, whose `//` can read as a comment; a Rust
// lifetime such as `'a`, which reads as an unclosed quote and hides the rest of its line; and a
// Rust string that spans lines, whose later lines read as code. Taken from Afterfund's prose-lint.
function scanComments(text) {
  const out = []
  let state = 'code'
  let quote = ''
  let buffer = ''
  let n = 1
  const flush = () => {
    if (buffer.trim()) out.push({ n, raw: buffer })
    buffer = ''
  }
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    const next = text[i + 1]
    if (c === '\n') {
      flush()
      n++
      // A quote can't span lines unless it's a template literal, so a stray apostrophe costs one line at most.
      if (state === 'line' || (state === 'string' && quote !== '`')) state = 'code'
      continue
    }
    if (state === 'code') {
      if (c === '/' && next === '/') {
        state = 'line'
        i++
      } else if (c === '/' && next === '*') {
        state = 'block'
        i++
      } else if (c === '"' || c === "'" || c === '`') {
        state = 'string'
        quote = c
      }
    } else if (state === 'string') {
      if (c === '\\') i++
      else if (c === quote) state = 'code'
    } else if (state === 'line') {
      buffer += c
    } else if (state === 'block') {
      if (c === '*' && next === '/') {
        state = 'code'
        i++
      } else buffer += c
    }
  }
  flush()
  // Drop doc-comment gutters and tags before reading the prose.
  return out.map(({ n, raw }) => ({ n, ...maskLine(raw.replace(/^\s*\*+/, '').replace(/^\s*[/!]\s?/, '').replace(/@\w+/g, '')) }))
}

// Returns each line of comment text in a TOML file, where a comment runs from a `#` outside a
// string to the line's end. A string that spans lines isn't recognized, so a `#` inside one reads
// as a comment.
function scanHashComments(text) {
  const out = []
  text.split('\n').forEach((line, i) => {
    let quote = ''
    for (let j = 0; j < line.length; j++) {
      const c = line[j]
      if (quote) {
        if (c === '\\' && quote === '"') j++
        else if (c === quote) quote = ''
      } else if (c === '"' || c === "'") {
        quote = c
      } else if (c === '#') {
        const raw = line.slice(j + 1)
        if (raw.trim()) out.push({ n: i + 1, ...maskLine(raw) })
        return
      }
    }
  })
  return out
}

function loadTermsInto(add) {
  try {
    return loadTerms()
  } catch (error) {
    add(1, error.message)
    return []
  }
}

function lintTerms(l, terms, add) {
  for (const term of terms) {
    for (const m of l.prose.matchAll(term.re)) add(l.n, `"${m[0]}": ${term.why}. Instead: ${term.instead}.`)
  }
  if (/[\p{L}\p{N})\]]!(?=\s|$)/u.test(l.prose)) add(l.n, 'Exclamation mark.')
  const emoji = l.prose.match(/(?![©®™])\p{Extended_Pictographic}/u)
  if (emoji) add(l.n, `Emoji ${emoji[0]}.`)
}

function lint(file, text, ranges) {
  if (CODE_FILE.test(file)) return lintCode(scanComments(text), ranges)
  if (HASH_FILE.test(file)) return lintCode(scanHashComments(text), ranges)
  return lintMarkdown(file, text, ranges)
}

function lintCode(comments, ranges) {
  const findings = []
  const add = (line, message) => findings.push({ line, message })
  const inRange = (n) => !ranges || ranges.some(([s, e]) => n >= s && n <= e)
  const terms = loadTermsInto(add)
  for (const l of comments) {
    if (!inRange(l.n)) continue
    lintTerms(l, terms, add)
    for (const m of l.prose.matchAll(HISTORY)) {
      add(l.n, `Comment "${m[0]}" tells the code's history, which git keeps. Say what holds, or delete it.`)
    }
  }
  return findings.sort((a, b) => a.line - b.line)
}

function lintMarkdown(file, text, ranges) {
  const findings = []
  const add = (line, message) => findings.push({ line, message })
  const inRange = (first, last = first) => !ranges || ranges.some(([s, e]) => first <= e && last >= s)
  const terms = loadTermsInto(add)
  const lines = scan(text)
  const slugCache = new Map()

  for (const l of lines) {
    if (!inRange(l.n)) continue
    lintTerms(l, terms, add)
    if (l.heading) {
      const title = l.prose.replace(/^#+\s*/, '').trim()
      if (title.endsWith('?')) add(l.n, 'Heading ends in a question mark. Name the content instead.')
      if (BANNED_HEADINGS.has(title.toLowerCase())) {
        add(l.n, `Heading "${title}". The first paragraph is the overview, and a closing summary repeats the document.`)
      }
    }
    for (const m of l.code.matchAll(/\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g)) {
      const problem = checkLink(file, m[1], slugCache)
      if (problem) add(l.n, problem)
    }
  }

  const units = new Map()
  for (const l of lines) {
    if (!units.has(l.unit)) units.set(l.unit, [])
    units.get(l.unit).push(l)
  }
  for (const group of units.values()) {
    const first = group[0].n
    if (!inRange(first, group[group.length - 1].n)) continue
    const joined = group.map((l) => l.prose).join('\n')
    const dashes = (joined.match(/—/g) || []).length
    if (dashes > 1) add(first, `${dashes} em dashes in one paragraph. Keep one at most.`)
    const bold = (joined.match(/\*\*[^*\n]+\*\*|__[^_\n]+__/g) || []).length
    if (bold > 1) add(first, `${bold} bold phrases in one paragraph. Bold only the one a skimmer must catch.`)
  }

  checkReferences(lines, add, inRange)

  return findings.sort((a, b) => a.line - b.line)
}

// A document with a "## Sources" section cites it with numbered references like [3]. A source
// added by hand can reuse a number already taken, and the citation meant for it then points at
// another source, with nothing failing.
function checkReferences(lines, add, inRange) {
  const start = lines.findIndex((l) => l.heading && /^#{1,6}\s+Sources\s*$/.test(l.code.trim()))
  if (start < 0) return
  let end = lines.findIndex((l, i) => i > start && l.heading)
  if (end < 0) end = lines.length

  const sources = new Map()
  for (const l of lines.slice(start + 1, end)) {
    const item = l.code.match(/^(\d+)\.\s/)
    if (!item) continue
    const n = Number(item[1])
    if (sources.has(n)) {
      if (inRange(l.n)) add(l.n, `Source ${n} is numbered twice, also at line ${sources.get(n)}.`)
    } else {
      sources.set(n, l.n)
    }
  }
  const numbers = [...sources.keys()].sort((a, b) => a - b)
  numbers.forEach((n, i) => {
    if (n !== i + 1 && inRange(sources.get(n))) add(sources.get(n), `Source ${n} breaks the sequence: expected ${i + 1}.`)
  })

  const cited = new Set()
  for (const l of lines) {
    for (const m of l.code.matchAll(/\[(\d+)\](?!\()/g)) {
      const n = Number(m[1])
      cited.add(n)
      if (!sources.has(n) && inRange(l.n)) add(l.n, `Reference [${n}] has no entry in the Sources list.`)
    }
  }
  for (const [n, line] of sources) {
    if (!cited.has(n) && inRange(line)) add(line, `Source ${n} is never cited.`)
  }
}

function checkLink(file, dest, cache) {
  if (/^[a-z][a-z0-9+.-]*:/i.test(dest) || dest.startsWith('//')) return null
  const [path, anchor] = dest.split('#')
  let target = file
  if (path) {
    // GitHub resolves a leading slash from the repository root.
    const decoded = decodeURIComponent(path)
    target = decoded.startsWith('/') ? resolve(ROOT, `.${decoded}`) : resolve(dirname(file), decoded)
    if (!existsSync(target)) return `Link target ${dest} doesn't exist.`
  }
  if (anchor && /\.md$/i.test(target)) {
    if (!cache.has(target)) cache.set(target, headingSlugs(readFileSync(target, 'utf8')))
    if (!cache.get(target).has(anchor.toLowerCase())) {
      return `Link anchor #${anchor} matches no heading in ${path || 'this file'}.`
    }
  }
  return null
}

// GitHub's anchor for a heading: lower case, punctuation other than - and _ dropped, each space a
// hyphen, and -1, -2 appended to repeats, so "## The C runtime" becomes #the-c-runtime.
function headingSlugs(text) {
  const seen = new Map()
  const slugs = new Set()
  let fence = null
  for (const line of text.replace(/\r\n/g, '\n').split('\n')) {
    const trimmed = line.trim()
    if (fence) {
      if (trimmed.startsWith(fence)) fence = null
      continue
    }
    const fenceOpen = trimmed.match(/^(```+|~~~+)/)
    if (fenceOpen) {
      fence = fenceOpen[1]
      continue
    }
    const heading = line.match(/^#{1,6}\s+(.*?)\s*#*\s*$/)
    if (!heading) continue
    const base = heading[1]
      .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
      .replace(/[`*]/g, '')
      .toLowerCase()
      .replace(/[^\p{L}\p{N}\s_-]/gu, '')
      .replace(/\s/g, '-')
    const count = seen.get(base) || 0
    seen.set(base, count + 1)
    slugs.add(count ? `${base}-${count}` : base)
  }
  return slugs
}

function rangeOf(text, piece) {
  const at = text.indexOf(piece)
  if (at < 0) return null
  const start = text.slice(0, at).split('\n').length
  return [start, start + piece.split('\n').length - 1]
}

async function readStdin() {
  const chunks = []
  for await (const chunk of process.stdin) chunks.push(chunk)
  // Windows PowerShell puts a byte order mark in front of what it pipes.
  return Buffer.concat(chunks).toString('utf8').replace(/^﻿/, '')
}

async function hook() {
  const payload = JSON.parse(await readStdin())
  const input = payload.tool_input || {}
  const file = input.file_path && resolve(input.file_path)
  if (!file || !(/\.md$/i.test(file) || CODE_FILE.test(file) || HASH_FILE.test(file)) || !existsSync(file)) return null
  const rel = relative(ROOT, file).replace(/\\/g, '/')
  if (rel.startsWith('..') || isAbsolute(rel) || SKIPPED.test(rel)) return null
  const text = readFileSync(file, 'utf8').replace(/\r\n/g, '\n')
  let ranges = null
  if (payload.tool_name !== 'Write') {
    const written = [input.new_string, ...(input.edits || []).map((edit) => edit.new_string)]
    ranges = written
      .filter(Boolean)
      .map((piece) => rangeOf(text, piece.replace(/\r\n/g, '\n')))
      .filter(Boolean)
    if (!ranges.length) return null
  }
  const findings = lint(file, text, ranges)
  if (!findings.length) return null
  const count = `${findings.length} finding${findings.length === 1 ? '' : 's'}`
  const lines = findings.slice(0, HOOK_MAX).map((f) => `  line ${f.line}: ${f.message}`)
  if (findings.length > HOOK_MAX) lines.push(`  and ${findings.length - HOOK_MAX} more`)
  return [
    `prose-lint: ${count} in ${rel}. The rules are in .claude/rules/writing.md. Fix each, or keep one on purpose.`,
    ...lines,
  ].join('\n')
}

const args = process.argv.slice(2)
if (args[0] === '--hook') {
  // A broken lint must not stop edits, but the agent should hear about it rather than lose the check silently.
  let context
  try {
    context = await hook()
  } catch (error) {
    context = `prose-lint failed and checked nothing: ${error.message}`
  }
  if (context) {
    process.stdout.write(JSON.stringify({ hookSpecificOutput: { hookEventName: 'PostToolUse', additionalContext: context } }))
  }
} else if (args.length) {
  let total = 0
  for (const arg of args) {
    const file = resolve(arg)
    const text = readFileSync(file, 'utf8').replace(/\r\n/g, '\n')
    for (const f of lint(file, text, null)) {
      console.log(`${relative(process.cwd(), file).replace(/\\/g, '/')}:${f.line}: ${f.message}`)
      total++
    }
  }
  process.exitCode = total ? 1 : 0
} else {
  console.error('usage: node tools/prose-lint.mjs <file>... | --hook')
  process.exitCode = 2
}
