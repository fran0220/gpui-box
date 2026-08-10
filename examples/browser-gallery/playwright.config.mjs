import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: ".",
  testMatch: "smoke.spec.mjs",
  timeout: 30_000,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 920, height: 1000 },
    deviceScaleFactor: 1,
    colorScheme: "light",
    reducedMotion: "reduce",
    screenshot: "off",
    launchOptions: {
      args: ["--enable-unsafe-webgpu"],
    },
  },
  webServer: {
    command: "node examples/browser-gallery/server.mjs",
    cwd: repoRoot,
    url: "http://127.0.0.1:4173/index.html",
    reuseExistingServer: false,
  },
});
