import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPOSITORY_ROOT = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const SENSITIVE_CONNECTION_URL_NAME = '(?:database_url|nats_url|connection_url|raw_connection_url)';
const LOG_OR_FORMAT_MACRO = /\b(tracing::(?:trace|debug|info|warn|error|event|trace_span|debug_span|info_span|warn_span|error_span)|format|format_args|anyhow|bail|ensure)!\s*(\(|\{|\[)/g;

const FORBIDDEN_RULES = [
  {
    rule: 'raw_database_url_field',
    pattern: /%\s*database_url\b/g,
  },
  {
    rule: 'raw_nats_url_field',
    pattern: /%\s*nats_url\b/g,
  },
  {
    rule: 'raw_connection_url_debug_field',
    pattern: /\?\s*(?:database_url|nats_url)\b/g,
  },
  {
    rule: 'raw_connection_url_alias_or_member_field',
    pattern: /[%?]\s*(?:&\s*)?(?:connection_url|raw_connection_url|(?:[A-Za-z_][A-Za-z0-9_]*\s*\.\s*)+(?:database_url|nats_url|connection_url|raw_connection_url))\b/g,
  },
  {
    rule: 'raw_connection_url_interpolation',
    pattern: /\{\s*(?:database_url|nats_url|connection_url|raw_connection_url)(?::[^}]*)?\s*\}/g,
  },
];

function characterLiteralEnd(source, openingIndex) {
  let index = openingIndex + 1;
  if (source[index] === '\\') {
    const escape = source[index + 1];
    if (escape === 'x') {
      index += 4;
    } else if (escape === 'u' && source[index + 2] === '{') {
      const closingBrace = source.indexOf('}', index + 3);
      if (closingBrace === -1) {
        return null;
      }
      index = closingBrace + 1;
    } else {
      index += 2;
    }
    return source[index] === "'" ? index : null;
  }

  const codePoint = source.codePointAt(index);
  if (codePoint === undefined) {
    return null;
  }
  index += codePoint > 0xffff ? 2 : 1;
  return source[index] === "'" ? index : null;
}

function matchingMacroDelimiter(source, openingIndex) {
  const closingFor = { '(': ')', '{': '}', '[': ']' };
  const openings = new Set(Object.keys(closingFor));
  const closings = new Set(Object.values(closingFor));
  const stack = [source[openingIndex]];
  let blockCommentDepth = 0;
  for (let index = openingIndex + 1; index < source.length; index += 1) {
    if (blockCommentDepth > 0) {
      if (source.startsWith('/*', index)) {
        blockCommentDepth += 1;
        index += 1;
      } else if (source.startsWith('*/', index)) {
        blockCommentDepth -= 1;
        index += 1;
      }
      continue;
    }

    if (source.startsWith('//', index)) {
      const newline = source.indexOf('\n', index + 2);
      if (newline === -1) {
        return null;
      }
      index = newline;
      continue;
    }
    if (source.startsWith('/*', index)) {
      blockCommentDepth = 1;
      index += 1;
      continue;
    }

    if (source[index] === 'r') {
      let quoteIndex = index + 1;
      while (source[quoteIndex] === '#') {
        quoteIndex += 1;
      }
      if (source[quoteIndex] === '"') {
        const terminator = `"${'#'.repeat(quoteIndex - index - 1)}`;
        const closing = source.indexOf(terminator, quoteIndex + 1);
        if (closing === -1) {
          return null;
        }
        index = closing + terminator.length - 1;
        continue;
      }
    }

    if (source[index] === '"') {
      for (index += 1; index < source.length; index += 1) {
        if (source[index] === '\\') {
          index += 1;
        } else if (source[index] === '"') {
          break;
        }
      }
      continue;
    }

    if (source[index] === "'") {
      const closing = characterLiteralEnd(source, index);
      if (closing !== null) {
        index = closing;
        continue;
      }
    }

    if (openings.has(source[index])) {
      stack.push(source[index]);
    } else if (closings.has(source[index])) {
      if (closingFor[stack.at(-1)] !== source[index]) {
        return null;
      }
      stack.pop();
      if (stack.length === 0) {
        return index;
      }
    }
  }
  return null;
}

function findMacroViolations(source, file) {
  const violations = [];
  LOG_OR_FORMAT_MACRO.lastIndex = 0;
  for (let match = LOG_OR_FORMAT_MACRO.exec(source); match; match = LOG_OR_FORMAT_MACRO.exec(source)) {
    const openingIndex = LOG_OR_FORMAT_MACRO.lastIndex - 1;
    const closingIndex = matchingMacroDelimiter(source, openingIndex);
    if (closingIndex === null) {
      continue;
    }
    const invocation = source.slice(match.index, closingIndex + 1);
    const line = lineNumberAt(source, match.index);

    if (match[1].startsWith('tracing::')) {
      const sensitiveField = new RegExp(`\\b${SENSITIVE_CONNECTION_URL_NAME}\\s*=`);
      if (sensitiveField.test(invocation)) {
        violations.push({ file, line, rule: 'raw_connection_url_named_field' });
      }
    }

    const positionalPlaceholder = /\{(?:\d+)?(?::[^}]*)?\}/;
    const sensitiveArgument = new RegExp(`\\b${SENSITIVE_CONNECTION_URL_NAME}\\b`);
    if (positionalPlaceholder.test(invocation) && sensitiveArgument.test(invocation)) {
      violations.push({ file, line, rule: 'raw_connection_url_positional_format' });
    }
  }
  return violations;
}

function lineNumberAt(source, index) {
  let line = 1;
  for (let offset = 0; offset < index; offset += 1) {
    if (source.charCodeAt(offset) === 10) {
      line += 1;
    }
  }
  return line;
}

export function findSensitiveConnectionLogViolations(source, file = '<memory>') {
  const violations = [];
  for (const { rule, pattern } of FORBIDDEN_RULES) {
    pattern.lastIndex = 0;
    for (let match = pattern.exec(source); match; match = pattern.exec(source)) {
      violations.push({
        file: file.replaceAll('\\', '/'),
        line: lineNumberAt(source, match.index),
        rule,
      });
    }
  }
  violations.push(...findMacroViolations(source, file.replaceAll('\\', '/')));
  return violations.sort((left, right) => (
    left.file.localeCompare(right.file)
      || left.line - right.line
      || left.rule.localeCompare(right.rule)
  ));
}

function rustFilesBelow(root) {
  const files = [];
  const pending = [root];
  while (pending.length > 0) {
    const current = pending.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const absolutePath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        pending.push(absolutePath);
      } else if (entry.isFile() && entry.name.endsWith('.rs')) {
        files.push(absolutePath);
      }
    }
  }
  return files.sort();
}

export function scanSensitiveConnectionLogFields(root) {
  const absoluteRoot = path.resolve(root);
  const violations = [];
  for (const file of rustFilesBelow(absoluteRoot)) {
    const relativePath = path.relative(absoluteRoot, file).replaceAll('\\', '/');
    const source = fs.readFileSync(file, 'utf8');
    violations.push(...findSensitiveConnectionLogViolations(source, relativePath));
  }
  return violations.sort((left, right) => (
    left.file.localeCompare(right.file)
      || left.line - right.line
      || left.rule.localeCompare(right.rule)
  ));
}

function runCli() {
  const scanRoot = process.argv[2]
    ? path.resolve(process.argv[2])
    : path.join(REPOSITORY_ROOT, 'crates');
  const violations = scanSensitiveConnectionLogFields(scanRoot);
  if (violations.length > 0) {
    for (const violation of violations) {
      console.error(`${violation.file}:${violation.line} ${violation.rule}`);
    }
    console.error(`Sensitive connection log field check failed with ${violations.length} violation(s).`);
    process.exitCode = 1;
    return;
  }
  console.log('Sensitive connection log field check passed.');
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : null;
if (invokedPath === fileURLToPath(import.meta.url)) {
  runCli();
}
