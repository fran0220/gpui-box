import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: ".",
  testMatch: "smoke.spec.mjs",
  // A cold CI worker can spend most of the default timeout starting
  // SwiftShader and compiling the first forced-WebGL scene. The same scene
  // completes in seconds once the renderer cache is warm, so allow cold-start
  // headroom without hiding failures behind retries.
  timeout: 60_000,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 920, height: 1000 },
    deviceScaleFactor: 1,
    colorScheme: "light",
    reducedMotion: "reduce",
    screenshot: "off",
  },
  projects: [
    {
      name: "forced-webgl",
      metadata: { backend: "webgl", renderer: "webgl2" },
      use: { launchOptions: { args: ["--enable-unsafe-swiftshader"] } },
    },
    {
      name: "auto-webgl-fallback",
      metadata: { backend: "auto", renderer: "webgl2" },
      use: {
        launchOptions: {
          args: ["--disable-features=WebGPU", "--enable-unsafe-swiftshader"],
        },
      },
    },
    {
      name: "forced-webgpu",
      metadata: { backend: "webgpu", renderer: "webgpu" },
      use: {
        launchOptions: {
          args: ["--enable-unsafe-webgpu", "--enable-unsafe-swiftshader"],
        },
      },
    },
  ],
  webServer: {
    command: "node examples/browser-gallery/server.mjs",
    cwd: repoRoot,
    url: "http://127.0.0.1:4173/index.html",
    reuseExistingServer: false,
  },
});
