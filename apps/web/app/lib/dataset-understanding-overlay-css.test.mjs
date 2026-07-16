import assert from 'node:assert/strict';
import fs from 'node:fs';
import { describe, it } from 'node:test';

const css = fs.readFileSync(new URL('../globals.css', import.meta.url), 'utf8');

function ruleBody(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`));
  assert.ok(match, `missing CSS rule: ${selector}`);
  return match[1];
}

function zIndex(body) {
  const match = body.match(/z-index:\s*(\d+)/);
  assert.ok(match, 'missing numeric z-index');
  return Number(match[1]);
}

describe('dataset understanding overlay stacking', () => {
  it('keeps the cross-dataset selector above later graph sections', () => {
    const siblingLayer = zIndex(ruleBody('.dataset-understanding-panel > *'));
    const toolbarLayer = zIndex(ruleBody('.dataset-understanding-mode-toolbar'));
    const dropdownLayer = zIndex(ruleBody('.dataset-understanding-cross-selection details > div'));

    assert.ok(toolbarLayer > siblingLayer);
    assert.ok(dropdownLayer > siblingLayer);
  });
});
