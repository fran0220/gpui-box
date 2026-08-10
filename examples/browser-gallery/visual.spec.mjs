import { expect, test } from "@playwright/test";

const requestedScenes = new Set(
  (process.env.GPUI_KIT_WEB_SCENES || "").split(",").filter(Boolean),
);

async function openScene(page, scene, theme) {
  await page.goto(`/index.html?scene=${scene}&theme=${theme}&backend=webgl`);
  await page.waitForFunction(() => window.gpuiKitGalleryReady === true);
  const canvas = page.locator("canvas");
  await expect(canvas).toHaveAttribute("data-gpui-renderer", "webgl2");

  // Match the native capture contract: park outside GPUI's logical viewport,
  // discard one warm-up frame, then let the following frame settle.
  await canvas.dispatchEvent("pointermove", {
    bubbles: true,
    buttons: 0,
    clientX: -1,
    clientY: -1,
    pointerId: 1,
    pointerType: "mouse",
  });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  return canvas;
}

test("canonical Rust scene catalog is visually stable in both bundled themes", async ({ page }) => {
  await openScene(page, "button", "studio-light");
  const catalog = JSON.parse(await page.evaluate(() => window.gpuiKitCatalog));
  const scenes = requestedScenes.size
    ? catalog.scenes.filter(scene => requestedScenes.has(scene))
    : catalog.scenes;
  expect(scenes).toHaveLength(requestedScenes.size || catalog.scenes.length);
  expect(catalog.themes.length).toBeGreaterThan(0);

  for (const theme of catalog.themes) {
    for (const scene of scenes) {
      const canvas = await openScene(page, scene, theme);
      expect(await canvas.screenshot({ animations: "disabled", caret: "hide", scale: "css" }))
        .toMatchSnapshot(`${theme}-${scene}.png`);
    }
  }
});
