function ssePayloadObject(payload) {
  return payload && typeof payload === 'object' && !Array.isArray(payload) ? payload : {};
}

function sseNestedData(payload) {
  const object = ssePayloadObject(payload);
  return ssePayloadObject(object.data);
}

export function assistantRunStreamDisplayText(eventName, payload) {
  if (!String(eventName || '').startsWith('assistant_run.')) return '';
  const normalizedEventName = String(eventName || '');
  if (normalizedEventName.endsWith('.delta') || normalizedEventName.endsWith('.completed')) {
    return '';
  }
  const object = ssePayloadObject(payload);
  const nested = sseNestedData(payload);
  return String(
    object.display_text
      || object.displayText
      || nested.display_text
      || nested.displayText
      || '',
  ).trim();
}

export function firstGeneratedArtifactUrlFromPayload(payload) {
  const candidates = [];
  const visit = (value, depth = 0) => {
    if (!value || depth > 4) return;
    if (typeof value === 'string') {
      if (
        /^https?:\/\/[^ ]+\/generated-artifacts\//.test(value)
        || value.startsWith('/generated-artifacts/')
      ) {
        candidates.push(value);
      }
      return;
    }
    if (Array.isArray(value)) {
      value.forEach((item) => visit(item, depth + 1));
      return;
    }
    if (typeof value !== 'object') return;
    [
      'artifact_links',
      'artifactLinks',
      'public_url',
      'publicUrl',
      'generated_artifact_url',
      'generatedArtifactUrl',
      'html_preview_url',
      'htmlPreviewUrl',
    ].forEach((key) => {
      if (Object.prototype.hasOwnProperty.call(value, key)) {
        visit(value[key], depth + 1);
      }
    });
    visit(value.card, depth + 1);
    visit(value.reply, depth + 1);
    visit(value.response, depth + 1);
    visit(value.data, depth + 1);
  };
  visit(payload);
  return candidates[0] || '';
}

export function assistantRunStreamArtifactLink(eventName, payload) {
  if (!String(eventName || '').startsWith('assistant_run.')) return '';
  return firstGeneratedArtifactUrlFromPayload(payload);
}

export function cleanAssistantVisibleContent(content) {
  let value = String(content || '')
    .trim()
    .replaceAll('\\r\\n', '\n')
    .replaceAll('\\n', '\n')
    .replaceAll('\\t', ' ');
  const markers = [
    '{"assistant_run_id"',
    '"assistant_run_id"',
    '{"card"',
    '{"conversation_external_id"',
    '"conversation_external_id"',
    '{"data":{"assistant_run_id"',
    '"idempotency_key"',
    '"poll_after_seconds"',
    '"status_url"',
  ];
  const markerIndexes = markers
    .map((marker) => value.indexOf(marker))
    .filter((index) => index >= 0);
  if (markerIndexes.length) {
    let cutAt = Math.min(...markerIndexes);
    const objectStart = value.slice(0, cutAt).lastIndexOf('{');
    if (objectStart >= 0) {
      cutAt = objectStart;
    }
    const prefix = value.slice(0, cutAt).trim();
    value = prefix;
  }
  const seenLinks = new Set();
  const lines = value.split(/\r?\n/).filter((line) => {
    const trimmed = line.trim();
    if (
      trimmed.startsWith('{"assistant_run_id"')
      || trimmed.startsWith('{"card"')
      || trimmed.startsWith('{"data":{"assistant_run_id"')
    ) {
      return false;
    }
    const link = trimmed
      .split(/\s+/)
      .find((part) => part.includes('/generated-artifacts/'));
    if (!link) return true;
    const normalized = link.replace(/[)\]>。，,;；]+$/u, '');
    if (seenLinks.has(normalized)) return false;
    seenLinks.add(normalized);
    return true;
  });
  const compact = [];
  let blankSeen = false;
  lines.forEach((line) => {
    const trimmedEnd = line.trimEnd();
    if (!trimmedEnd.trim()) {
      if (!blankSeen) compact.push('');
      blankSeen = true;
      return;
    }
    compact.push(trimmedEnd);
    blankSeen = false;
  });
  return compact.join('\n').trim();
}

export function appendArtifactLinkText(content, artifactUrl) {
  const url = String(artifactUrl || '').trim();
  const cleanContent = cleanAssistantVisibleContent(content);
  if (!url || cleanContent.includes(url)) return cleanContent || '';
  return `${cleanContent || '页面已生成。'}\n\n[打开生成页面](${url})`;
}
