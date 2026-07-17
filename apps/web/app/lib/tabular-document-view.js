const TABULAR_CONTENT_HINTS = ['csv', 'tsv', 'spreadsheet', 'excel', 'sheet'];

function normalizedCell(value) {
  return String(value ?? '').replace(/^\uFEFF/, '').trim();
}

function delimiterScore(line, delimiter) {
  let count = 0;
  let quoted = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (character === '"') {
      if (quoted && line[index + 1] === '"') {
        index += 1;
      } else {
        quoted = !quoted;
      }
    } else if (!quoted && character === delimiter) {
      count += 1;
    }
  }
  return count;
}

export function inferTabularDelimiter(rawText = '') {
  const text = String(rawText || '');
  const scanLimit = Math.min(text.length, 8_192);
  let line = '';
  let current = '';
  for (let index = 0; index < scanLimit; index += 1) {
    const character = text[index];
    if (character === '\n' || character === '\r') {
      if (current.trim()) {
        line = current.trim();
        break;
      }
      current = '';
    } else {
      current += character;
    }
  }
  if (!line) line = current.trim();
  const candidates = [',', '\t', ';'];
  const ranked = candidates
    .map((delimiter) => ({ delimiter, score: delimiterScore(line, delimiter) }))
    .sort((left, right) => right.score - left.score);
  return ranked[0]?.score > 0 ? ranked[0].delimiter : '';
}

const SENSITIVE_COLUMN_NAMES = new Set([
  'pid',
  'personid',
  'personidentifier',
  'faceid',
  'visitorid',
  'customerid',
  'memberid',
  'userid',
  'openid',
  'unionid',
]);

const SENSITIVE_COLUMN_HINTS = [
  'phone',
  'mobile',
  'email',
  'idcard',
  'identitycard',
  'token',
  'secret',
  'password',
  'credential',
  'cookie',
];

function normalizedColumnKey(column = '') {
  return String(column || '').toLowerCase().replace(/[^a-z0-9]/g, '');
}

export function isSensitiveTabularColumn(column = '') {
  const key = normalizedColumnKey(column);
  return SENSITIVE_COLUMN_NAMES.has(key)
    || SENSITIVE_COLUMN_HINTS.some((hint) => key.includes(hint));
}

function looksSensitiveTabularValue(value = '') {
  const text = String(value || '').trim();
  if (!text) return false;
  if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text)) return true;
  if (/^(?:\+?86[- ]?)?1[3-9]\d{9}$/.test(text.replace(/[()]/g, ''))) return true;
  if (/^\d{17}[\dXx]$/.test(text)) return true;
  if (/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(text)) return true;
  return text.length >= 24
    && /^[A-Za-z0-9+/_=-]+$/.test(text)
    && /[A-Za-z]/.test(text)
    && /\d/.test(text);
}

export function maskTabularCell(column, value) {
  const normalized = normalizedCell(value);
  const masked = isSensitiveTabularColumn(column) || looksSensitiveTabularValue(normalized);
  return {
    value: masked && normalized ? '已隐藏' : normalized,
    masked,
  };
}

function uniqueColumns(values = [], maxColumns = 40) {
  const seen = new Map();
  return values.slice(0, maxColumns).map((value, index) => {
    const base = normalizedCell(value) || `列${index + 1}`;
    const count = (seen.get(base) || 0) + 1;
    seen.set(base, count);
    return count === 1 ? base : `${base}_${count}`;
  });
}

export function parseDelimitedPreview(rawText = '', {
  delimiter = '',
  maxRows = 20,
  maxColumns = 40,
} = {}) {
  const text = String(rawText || '').replace(/^\uFEFF/, '');
  const resolvedDelimiter = delimiter || inferTabularDelimiter(text);
  if (!text || !resolvedDelimiter) {
    return null;
  }

  const parsedRows = [];
  let row = [];
  let cell = '';
  let quoted = false;
  const normalizedMaxRows = Math.max(1, Math.floor(Number(maxRows) || 20));
  const scanRowLimit = (normalizedMaxRows * 4) + 1;

  const finishCell = () => {
    row.push(normalizedCell(cell));
    cell = '';
  };
  const finishRow = () => {
    finishCell();
    if (row.some(Boolean)) {
      parsedRows.push(row.slice(0, maxColumns));
    }
    row = [];
  };

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (character === '"') {
      if (quoted && text[index + 1] === '"') {
        cell += '"';
        index += 1;
      } else {
        quoted = !quoted;
      }
    } else if (!quoted && character === resolvedDelimiter) {
      finishCell();
    } else if (!quoted && (character === '\n' || character === '\r')) {
      if (character === '\r' && text[index + 1] === '\n') {
        index += 1;
      }
      finishRow();
      if (parsedRows.length >= scanRowLimit) break;
    } else {
      cell += character;
    }
  }
  if ((cell || row.length) && parsedRows.length < scanRowLimit) {
    finishRow();
  }

  if (parsedRows.length < 2 || parsedRows[0].length < 2) {
    return null;
  }
  const headerCells = parsedRows[0].slice(0, maxColumns).map(normalizedCell);
  const columns = uniqueColumns(headerCells, maxColumns);
  let hasSensitiveData = columns.some(isSensitiveTabularColumn);
  const rows = parsedRows
    .slice(1)
    .filter((values) => !headerCells.every((column, index) => normalizedCell(values[index]) === column))
    .slice(0, normalizedMaxRows)
    .map((values) => columns.map((column, index) => {
      const cell = maskTabularCell(column, values[index]);
      if (cell.masked) hasSensitiveData = true;
      return cell.value;
    }));
  if (!rows.length) {
    return null;
  }
  return {
    delimiter: resolvedDelimiter,
    columns,
    rows,
    rowLimit: normalizedMaxRows,
    hasSensitiveData,
  };
}

export function isTabularDocument(document = {}) {
  const sourceHint = [
    document?.content_type,
    document?.contentType,
    document?.title,
    document?.object_key,
    document?.objectKey,
  ].map((value) => String(value || '').toLowerCase()).join(' ');
  return TABULAR_CONTENT_HINTS.some((hint) => sourceHint.includes(hint));
}

export function buildTabularDocumentPreview(document = {}, rawText = '', options = {}) {
  if (!isTabularDocument(document)) return null;
  return parseDelimitedPreview(rawText, options);
}
