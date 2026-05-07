#!/usr/bin/env node
import http from "node:http";
import { spawn } from "node:child_process";

const port = Number(process.env.CODEX_HOST_SHIM_PORT || 54323);
const mode = process.env.CODEX_HOST_SHIM_MODE || "fake";
const model = process.env.CODEX_HOST_SHIM_MODEL || "MiniMax-M2.7";
const baseUrl = process.env.CODEX_HOST_SHIM_MINIMAX_BASE_URL || "https://api.minimaxi.com/v1";
const expected = process.env.CODEX_HOST_SHIM_EXPECTED || "CODEX_HOST_SMOKE_OK";
const codexBin = process.env.CODEX_HOST_SHIM_CODEX_BIN || "codex";
const codexJs = process.env.CODEX_HOST_SHIM_CODEX_JS || "";
const sandbox = process.env.CODEX_HOST_SHIM_SANDBOX || "read-only";
const jsonEvents = ["1", "true", "yes"].includes(
  String(process.env.CODEX_HOST_SHIM_JSON || "").toLowerCase(),
);

let requestCount = 0;
let lastRequest = "";

function sanitize(text) {
  return String(text || "").replace(/[A-Za-z0-9_-]{20,}/g, "[redacted]");
}

function responsePayload(text) {
  return {
    id: "resp_codex_host_smoke",
    object: "response",
    created_at: Math.floor(Date.now() / 1000),
    status: "completed",
    model,
    output: [
      {
        id: "msg_codex_host_smoke",
        type: "message",
        status: "completed",
        role: "assistant",
        content: [{ type: "output_text", text }],
      },
    ],
    usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 },
  };
}

function writeSse(res, name, payload) {
  res.write(`event: ${name}\n`);
  res.write(`data: ${JSON.stringify({ type: name, ...payload })}\n\n`);
}

function sendResponses(res, requestBody, text) {
  const id = "resp_codex_host_smoke";
  const itemId = "msg_codex_host_smoke";
  const stream = requestBody?.stream !== false;
  if (!stream) {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(responsePayload(text)));
    return;
  }

  const completed = responsePayload(text);
  res.writeHead(200, {
    "content-type": "text/event-stream",
    "cache-control": "no-cache",
    connection: "keep-alive",
  });
  writeSse(res, "response.created", {
    response: { id, object: "response", status: "in_progress", output: [] },
  });
  writeSse(res, "response.output_item.added", {
    response_id: id,
    output_index: 0,
    item: { id: itemId, type: "message", status: "in_progress", role: "assistant", content: [] },
  });
  writeSse(res, "response.content_part.added", {
    response_id: id,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    part: { type: "output_text", text: "" },
  });
  writeSse(res, "response.output_text.delta", {
    response_id: id,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    delta: text,
  });
  writeSse(res, "response.output_text.done", {
    response_id: id,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    text,
  });
  writeSse(res, "response.content_part.done", {
    response_id: id,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    part: { type: "output_text", text },
  });
  writeSse(res, "response.output_item.done", {
    response_id: id,
    output_index: 0,
    item: completed.output[0],
  });
  writeSse(res, "response.completed", { response: completed });
  res.write("data: [DONE]\n\n");
  res.end();
}

function flattenInput(value) {
  if (!value) return "";
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map(flattenInput).filter(Boolean).join("\n");
  if (typeof value === "object") {
    if (typeof value.text === "string") return value.text;
    if (typeof value.content === "string") return value.content;
    if (Array.isArray(value.content)) return flattenInput(value.content);
    if (Array.isArray(value.input)) return flattenInput(value.input);
  }
  return "";
}

async function completeWithMiniMax(requestBody) {
  const apiKey = process.env.MINIMAX_API_KEY;
  if (!apiKey) {
    throw new Error("MINIMAX_API_KEY is required for minimax mode");
  }
  const prompt = [
    flattenInput(requestBody?.instructions),
    flattenInput(requestBody?.input),
  ]
    .filter(Boolean)
    .join("\n")
    .trim() || `Reply exactly ${expected}`;
  const response = await fetch(`${baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${apiKey}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      model,
      messages: [{ role: "user", content: prompt }],
      temperature: 0,
    }),
  });
  if (!response.ok) {
    throw new Error(`MiniMax returned HTTP ${response.status}`);
  }
  const payload = await response.json();
  return String(payload?.choices?.[0]?.message?.content || "")
    .replace(/^\s*<think>[\s\S]*?<\/think>\s*/, "")
    .trim();
}

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", (chunk) => {
    body += chunk;
  });
  req.on("end", async () => {
    requestCount += 1;
    lastRequest = `${req.method} ${req.url}\n${body}`;
    if (req.method !== "POST" || req.url !== "/v1/responses") {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "not_found" }));
      return;
    }
    try {
      const requestBody = JSON.parse(body || "{}");
      const text = mode === "minimax" ? await completeWithMiniMax(requestBody) : expected;
      sendResponses(res, requestBody, text || expected);
    } catch (error) {
      res.writeHead(500, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: sanitize(error.message) }));
    }
  });
});

server.listen(port, "127.0.0.1", () => {
  const prompt = `Reply exactly ${expected}`;
  const args = [
    "-a",
    "never",
    "-c",
    'model_provider="localshim"',
    "-c",
    `model="${model}"`,
    "-c",
    'model_providers.localshim.name="localshim"',
    "-c",
    `model_providers.localshim.base_url="http://127.0.0.1:${port}/v1"`,
    "-c",
    'model_providers.localshim.env_key="MINIMAX_API_KEY"',
    "-c",
    'model_providers.localshim.wire_api="responses"',
    "-c",
    "model_providers.localshim.requires_openai_auth=false",
    "exec",
    "--ignore-user-config",
    "--skip-git-repo-check",
    "--ephemeral",
    "--sandbox",
    sandbox,
    "-",
  ];
  if (jsonEvents) {
    args.splice(args.indexOf("-"), 0, "--json");
  }
  const childProgram = codexJs ? process.execPath : codexBin;
  const childArgs = codexJs ? [codexJs, ...args] : args;
  const child = spawn(childProgram, childArgs, {
    env: { ...process.env, MINIMAX_API_KEY: process.env.MINIMAX_API_KEY || "dummy" },
  });
  let stdout = "";
  let stderr = "";
  let timedOut = false;
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });
  child.stdin.end(prompt);
  const timer = setTimeout(() => {
    timedOut = true;
    child.kill("SIGTERM");
  }, Number(process.env.CODEX_HOST_SHIM_TIMEOUT_MS || 120000));
  child.on("close", (status, signal) => {
    clearTimeout(timer);
    const output = sanitize(`${stdout}\n${stderr}`).slice(0, 5000);
    console.log(`CODEX_SHIM_MODE=${mode}`);
    console.log(`CODEX_SHIM_STATUS=${status}`);
    console.log(`CODEX_SHIM_SIGNAL=${signal || ""}`);
    console.log(`CODEX_SHIM_ERROR=${timedOut ? "timeout" : "none"}`);
    console.log(`CODEX_SHIM_REQUEST_COUNT=${requestCount}`);
    console.log("CODEX_SHIM_LAST_REQUEST_BEGIN");
    console.log(sanitize(lastRequest).slice(0, 2000));
    console.log("CODEX_SHIM_LAST_REQUEST_END");
    console.log("CODEX_SHIM_OUTPUT_BEGIN");
    console.log(output);
    console.log("CODEX_SHIM_OUTPUT_END");
    server.close(() => process.exit(status || (timedOut ? 3 : 0)));
  });
});
