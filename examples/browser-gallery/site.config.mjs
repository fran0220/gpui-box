import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: ".",
  testMatch: "site.spec.mjs",
  timeout: 90_000,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1280, height: 1000 },
    deviceScaleFactor: 1,
    colorScheme: "dark",
    reducedMotion: "reduce",
    screenshot: "off",
    launchOptions: {
      args: ["--disable-features=WebGPU", "--enable-unsafe-swiftshader"],
    },
  },
  webServer: {
    command: "node examples/browser-gallery/server.mjs target/site-browser-smoke",
    cwd: repoRoot,
    url: "http://127.0.0.1:4173/",
    reuseExistingServer: false,
  },
});
