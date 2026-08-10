import { createReadStream } from "node:fs";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../target/browser-gallery/", import.meta.url));
const types = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

createServer((request, response) => {
  const pathname = new URL(request.url, "http://127.0.0.1").pathname;
  const relative = pathname === "/" ? "index.html" : pathname.slice(1);
  const path = normalize(join(root, relative));
  if (!path.startsWith(root)) {
    response.writeHead(404).end();
    return;
  }
  const stream = createReadStream(path);
  stream.on("open", () => {
    response.writeHead(200, {
      "Content-Type": types.get(extname(path)) || "application/octet-stream",
      "Cache-Control": "no-store",
    });
    stream.pipe(response);
  });
  stream.on("error", () => response.writeHead(404).end());
}).listen(4173, "127.0.0.1");
