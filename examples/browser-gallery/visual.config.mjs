import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: ".",
  testMatch: "visual.spec.mjs",
  timeout: 30 * 60_000,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 920, height: 1000 },
    deviceScaleFactor: 1,
    colorScheme: "light",
    reducedMotion: "reduce",
    screenshot: "off",
  },
  expect: {
    toMatchSnapshot: {
      maxDiffPixels: 0,
    },
  },
  snapshotPathTemplate: `${repoRoot}/snapshots/browser/{arg}{ext}`,
  projects: [
    {
      name: "webgl2-visual",
      use: {
        launchOptions: { args: ["--enable-unsafe-swiftshader"] },
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
