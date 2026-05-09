'use client';

import { useEffect, useMemo } from 'react';
import {
  isAllowedHtmlArtifactMessage,
  renderHtmlArtifactDocument,
} from '../../lib/html-artifact-manifest';

export default function HtmlArtifactViewer({
  artifact,
  compact = false,
  onArtifactEvent,
}) {
  const rendered = useMemo(() => renderHtmlArtifactDocument(artifact || {}), [artifact]);

  useEffect(() => {
    if (!artifact || rendered.rejected || rendered.manifest?.interactionMode === 'read_only') {
      return undefined;
    }
    function handleMessage(event) {
      if (!isAllowedHtmlArtifactMessage(event.data, artifact)) {
        return;
      }
      onArtifactEvent?.(event.data, rendered.manifest);
    }
    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, [artifact, onArtifactEvent, rendered.manifest, rendered.rejected]);

  if (!artifact) {
    return (
      <div className="html-artifact-empty">
        <strong>暂无 HTML 产物</strong>
        <p>Codex 执行报告、静态页规划交接和代码审查摘要生成后，会在这里打开。</p>
      </div>
    );
  }

  if (rendered.rejected) {
    return (
      <div className="html-artifact-rejected">
        <strong>HTML 产物已拦截</strong>
        <p>{rendered.reason || 'manifest 未通过安全规则。'}</p>
      </div>
    );
  }

  return (
    <div className={`html-artifact-viewer ${compact ? 'compact' : ''}`.trim()}>
      <div className="html-artifact-toolbar">
        <div>
          <span>{rendered.manifest.templateLabel}</span>
          <strong>{rendered.manifest.title}</strong>
        </div>
        <em>{rendered.manifest.interactionMode === 'read_only' ? '只读沙箱' : '可提交意图'}</em>
      </div>
      <iframe
        className="html-artifact-frame"
        title={rendered.manifest.title}
        srcDoc={rendered.html}
        sandbox={rendered.sandbox}
      />
    </div>
  );
}
