import { defineConfig } from "@playwright/test";
import desktop from "./playwright.config.mjs";

// Chromium device emulation, not native Android Chrome or iOS Safari evidence.
export default defineConfig({
  ...desktop,
  testMatch: "mobile.spec.mjs",
  use: { ...desktop.use, viewport: { width: 390, height: 844 }, deviceScaleFactor: 2,
    isMobile: true, hasTouch: true },
  projects: [{ name: "mobile-chromium-webgl2", use: {
    launchOptions: { args: ["--enable-unsafe-swiftshader"] },
  } }],
});
