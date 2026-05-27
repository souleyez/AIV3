import crypto from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

export const runtime = 'nodejs';

const REPO_ROOT = path.resolve(process.cwd(), '..', '..');
const DEFAULT_UPLOAD_DIR = path.join(REPO_ROOT, 'storage', 'uploads');
const DEFAULT_UPLOAD_MAX_BYTES = 200 * 1024 * 1024;

function readUploadMaxBytes() {
  const raw = Number.parseInt(process.env.AIDP_V3_UPLOAD_MAX_BYTES || '', 10);
  return Number.isFinite(raw) && raw > 0 ? raw : DEFAULT_UPLOAD_MAX_BYTES;
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

function sanitizeFileName(fileName) {
  return String(fileName || 'upload.bin')
    .replace(/[\\/:*?"<>|]+/g, '_')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 160) || `upload-${Date.now()}.bin`;
}

function toWorkerReadablePath(filePath) {
  const normalized = path.resolve(filePath);
  const winDriveMatch = normalized.match(/^([A-Za-z]):\\(.*)$/);
  if (!winDriveMatch) {
    return normalized;
  }

  const drive = winDriveMatch[1].toLowerCase();
  const rest = winDriveMatch[2].replace(/\\/g, '/');
  return `/mnt/${drive}/${rest}`;
}

function inferContentType(file) {
  return file.type || 'application/octet-stream';
}

export async function POST(request) {
  try {
    const formData = await request.formData();
    const files = formData
      .getAll('files')
      .filter((item) => item && typeof item.arrayBuffer === 'function');

    if (!files.length) {
      return Response.json(
        {
          error: 'no_files_uploaded',
          message: '没有收到可保存的上传文件。',
        },
        { status: 400 },
      );
    }

    const uploadDir = path.resolve(process.env.AIDP_V3_UPLOAD_DIR || DEFAULT_UPLOAD_DIR);
    await mkdir(uploadDir, { recursive: true });

    const savedFiles = [];
    const maxBytes = readUploadMaxBytes();
    for (const [index, file] of files.entries()) {
      const originalName = sanitizeFileName(file.name);
      const declaredSize = Number(file.size || 0);
      if (declaredSize > maxBytes) {
        return Response.json(
          {
            error: 'upload_file_too_large',
            message: `文件 ${originalName} 大小 ${formatBytes(declaredSize)}，超过本地上传上限 ${formatBytes(maxBytes)}。`,
          },
          { status: 413 },
        );
      }
      const storedName = `${Date.now()}-${index + 1}-${crypto.randomUUID().slice(0, 8)}-${originalName}`;
      const targetPath = path.join(uploadDir, storedName);
      const bytes = Buffer.from(await file.arrayBuffer());
      if (bytes.byteLength > maxBytes) {
        return Response.json(
          {
            error: 'upload_file_too_large',
            message: `文件 ${originalName} 大小 ${formatBytes(bytes.byteLength)}，超过本地上传上限 ${formatBytes(maxBytes)}。`,
          },
          { status: 413 },
        );
      }
      await writeFile(targetPath, bytes);
      savedFiles.push({
        name: originalName,
        stored_name: storedName,
        size: bytes.byteLength,
        content_type: inferContentType(file),
        local_path: targetPath,
        object_key: toWorkerReadablePath(targetPath),
      });
    }

    return Response.json({
      files: savedFiles,
      upload_dir: uploadDir,
    });
  } catch (error) {
    return Response.json(
      {
        error: 'local_upload_failed',
        message: error instanceof Error ? error.message : '保存上传文件失败。',
      },
      { status: 500 },
    );
  }
}
