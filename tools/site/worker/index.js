// The catalog, on the public internet.
//
// One Worker serves two things that are really one thing: a site a person
// reads at `/` and `/mcp/`, and an MCP endpoint an agent POSTs at `/mcp`.
// Both answer out of `api-index.json`, which is generated from the source and
// checked by the gate, so the page and the tool cannot disagree about a
// signature.
//
// It renders nothing. `render_scene` returns the capture the gate already
// committed, and that is not a downgrade: scene captures in this repository
// are deterministic, so the bytes a renderer would produce here are the bytes
// already in `snapshots/`. Running a GPU-less software rasterizer per request
// would add cost, latency and an attack surface to produce an identical file.
//
// What it therefore cannot do is show a component you are in the middle of
// changing. That is the local stdio server's job, and `initialize` says so
// rather than letting a caller assume otherwise.

import TOOLS from "./tools.json";

const PROTOCOL = "2025-06-18";
const JSON_HEADERS = { "content-type": "application/json" };

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    if (url.pathname === "/mcp") {
      return mcp(request, env);
    }
    if (url.pathname === "/mcp/") {
      if (request.method === "GET" || request.method === "HEAD") {
        return env.ASSETS.fetch(request);
      }
      return mcp(request, env);
    }
    return env.ASSETS.fetch(request);
  },
};

async function mcp(request, env) {
  if (request.method === "GET") {
    // Streamable HTTP allows a server to decline the optional SSE stream.
    // Declining is honest here: nothing on this server pushes.
    return new Response("This endpoint answers POSTed JSON-RPC. It opens no stream.", {
      status: 405,
      headers: { allow: "POST" },
    });
  }
  if (request.method !== "POST") {
    return new Response(null, { status: 405, headers: { allow: "POST" } });
  }

  let body;
  try {
    body = await request.json();
  } catch {
    return reply(null, { code: -32700, message: "not JSON" });
  }

  const batch = Array.isArray(body) ? body : [body];
  const results = [];
  for (const message of batch) {
    // A notification carries no id, and answering one is a protocol error.
    if (message.id === undefined || message.id === null) continue;
    try {
      results.push({
        jsonrpc: "2.0",
        id: message.id,
        result: await dispatch(message.method, message.params ?? {}, env),
      });
    } catch (error) {
      results.push({
        jsonrpc: "2.0",
        id: message.id,
        error: { code: -32603, message: String(error.message ?? error) },
      });
    }
  }

  if (results.length === 0) return new Response(null, { status: 202 });
  return new Response(JSON.stringify(Array.isArray(body) ? results : results[0]), {
    headers: JSON_HEADERS,
  });
}

async function dispatch(method, params, env) {
  switch (method) {
    case "initialize":
      return {
        protocolVersion: PROTOCOL,
        capabilities: { tools: {} },
        serverInfo: { name: "gpui-box", version: VERSION },
        instructions:
          "The GPUI Box Kit component catalog, served from the current " +
          "repository catalog (this deployment of main, ahead of crates.io). " +
          "Signatures come from an index generated out of the source, and " +
          "render_scene returns the image the gate captured for that scene. " +
          "It cannot show uncommitted local edits — run the stdio server in " +
          "a working copy for that. Prefer these tools over recall.",
      };
    case "tools/list":
      return { tools: TOOLS };
    case "tools/call":
      return call(params, env);
    case "ping":
      return {};
    default:
      throw new Error(`unknown method: ${method}`);
  }
}

async function call({ name, arguments: args = {} }, env) {
  const index = await catalog(env);
  switch (name) {
    case "search_components":
      return text(search(index, args.query ?? "", args.kind ?? ""));
    case "component":
      return text(component(index, args.name ?? ""));
    case "scene":
      return text(scene(index, args.name ?? ""));
    case "rules":
      return text(await asset(env, "/llms.txt"));
    case "render_scene":
      return image(env, args.name ?? "", args.theme ?? "studio-dark");
    default:
      throw new Error(`unknown tool: ${name}`);
  }
}

const text = (body) => ({ content: [{ type: "text", text: body }] });

// ---------------------------------------------------------------------------
// The index, fetched from the same static assets the site is built from
// ---------------------------------------------------------------------------

let cached = null;

async function catalog(env) {
  if (cached) return cached;
  cached = JSON.parse(await asset(env, "/api-index.json"));
  return cached;
}

async function asset(env, path) {
  const response = await env.ASSETS.fetch(new Request(`https://assets.local${path}`));
  if (!response.ok) throw new Error(`missing asset ${path}`);
  return response.text();
}

function matchesQuery(entry, words) {
  if (words.length === 0) return true;
  const haystack = `${entry.name} ${entry.summary} ${entry.path}`.toLowerCase();
  return words.every((word) => haystack.includes(word));
}

function search(index, query, kind) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const includeComponents = !kind || kind === "builder" || kind === "view";
  const includeTypes = kind === "type" || (!kind && query.length > 0);
  const lines = [];

  if (includeComponents) {
    for (const entry of index.components) {
      if ((kind === "builder" || kind === "view") && entry.kind !== kind) continue;
      if (!matchesQuery(entry, words)) continue;
      lines.push(
        `${entry.name} (${entry.kind}) — ${entry.summary || "(no summary)"}\n  path: ${entry.path}\n  scenes: ${(entry.scenes || []).join(", ")}`,
      );
    }
  }
  if (includeTypes) {
    for (const entry of index.types || []) {
      if (!matchesQuery(entry, words)) continue;
      lines.push(
        `${entry.name} (type) — ${entry.summary || "(no summary)"}\n  path: ${entry.path}`,
      );
    }
  }

  if (lines.length === 0) {
    return `Nothing matches ${JSON.stringify(query)}. Search with one word, kind=type for supporting types, or an empty query to list all ${index.components.length} components.`;
  }
  return `${lines.length} match(es)\n\n${lines.join("\n\n")}`;
}

function section(title, values) {
  return values && values.length ? `\n${title}:\n${values.map((v) => `  ${v}`).join("\n")}\n` : "";
}

function component(index, name) {
  const found = index.components.find((entry) => entry.name === name);
  if (found) {
    let out = `${found.name} (${found.kind})\n${found.summary || "(no summary)"}\n\npath:   ${found.path}\nsource: ${found.source}\n`;
    out +=
      found.kind === "view"
        ? "\nA view survives a frame: hold it in an Entity with cx.new(...) and reach it with .update(...).\n"
        : "\nA builder is RenderOnce: construct and mount it in one expression.\n";
    out += section("construct", found.construct);
    out += section("options (chain onto the value)", found.options);
    out += section("commands (need a Context)", found.commands);
    out += section("queries", found.queries);
    out += section("reports", found.reports);
    if (found.scenes && found.scenes.length) {
      out += `\nscenes that render it: ${found.scenes.join(", ")}\nCall scene(name) for verified example code, or render_scene(name) to look at it.\n`;
    }
    return out;
  }

  const ty = (index.types || []).find((entry) => entry.name === name);
  if (ty) {
    let out = `${ty.name} (type)\n${ty.summary || "(no summary)"}\n\npath: ${ty.path}\n\nA supporting type: construct it and pass it to a component. It is not mounted on its own.\n`;
    out += section("variants", ty.variants);
    out += section("construct", ty.construct);
    out += section("options (chain onto the value)", ty.options);
    out += section("commands (need a Context)", ty.commands);
    out += section("queries", ty.queries);
    return out;
  }

  const close = [...index.components, ...(index.types || [])]
    .map((entry) => entry.name)
    .filter(
      (candidate) =>
        candidate.toLowerCase().includes(name.toLowerCase()) ||
        name.toLowerCase().includes(candidate.toLowerCase()),
    );
  throw new Error(
    close.length
      ? `no component or type named ${JSON.stringify(name)}. Did you mean: ${close.join(", ")}?`
      : `no component or type named ${JSON.stringify(name)}. Call search_components to find one.`,
  );
}

function scene(index, name) {
  const found = index.scenes.find((entry) => entry.name === name);
  if (!found) {
    throw new Error(
      `no scene named ${JSON.stringify(name)}. A component's scenes are listed by component(name).`,
    );
  }
  return `scene ${found.name}\nuses: ${found.uses.join(", ")}\n\n${found.example}\n`;
}

async function image(env, name, theme) {
  if (!name) throw new Error("render_scene needs a scene name");
  if (theme !== "studio-dark" && theme !== "studio-light") {
    throw new Error(`unknown theme ${JSON.stringify(theme)}: expected studio-dark or studio-light`);
  }

  const version = (await asset(env, "/image-version.txt")).trim();
  const path = `/images/${version}/${name}-${theme}.png`;
  const response = await env.ASSETS.fetch(new Request(`https://assets.local${path}`));
  if (!response.ok) {
    throw new Error(
      `no capture for ${JSON.stringify(name)} in ${theme}. Call search_components to find a scene.`,
    );
  }

  const bytes = new Uint8Array(await response.arrayBuffer());
  return {
    content: [
      { type: "text", text: `${name} in ${theme}, ${bytes.length} bytes, as the gate captured it` },
      { type: "image", mimeType: "image/png", data: base64(bytes) },
    ],
  };
}

// `btoa` takes a binary string, and building one megabyte-long with spread
// would blow the stack, so the bytes go in fixed chunks.
function base64(bytes) {
  let binary = "";
  const step = 0x8000;
  for (let at = 0; at < bytes.length; at += step) {
    binary += String.fromCharCode.apply(null, bytes.subarray(at, at + step));
  }
  return btoa(binary);
}

function reply(id, error) {
  return new Response(JSON.stringify({ jsonrpc: "2.0", id, error }), {
    status: 400,
    headers: JSON_HEADERS,
  });
}

const VERSION = "0.1.2";
