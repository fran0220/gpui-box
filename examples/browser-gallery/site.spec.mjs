import { expect, test } from "@playwright/test";

async function liveFrame(page, scene) {
  const host = page.locator(`[data-live-scene="${scene}"]`);
  const iframe = host.locator("iframe");
  await expect(iframe).toHaveAttribute("loading", "lazy");
  await expect(iframe).toHaveAttribute("title", `Live GPUI Box ${scene} scene`);
  await expect(host).toHaveClass(/is-ready/);
  await expect(host.locator(".live-fallback")).toBeHidden();
  const handle = await iframe.elementHandle();
  const frame = await handle.contentFrame();
  await frame.waitForFunction(() => window.gpuiKitGalleryReady === true);
  await expect(frame.locator("canvas")).toHaveAttribute("data-gpui-renderer", "webgl2");
  return frame;
}

test("static catalog pages retain their content while GPUI enhances them", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/GPUI Box/);
  await expect(page.getByRole("heading", { name: "Native desktop components for GPUI." }))
    .toBeVisible();
  await expect(page.getByRole("link", { name: "Playground", exact: true }))
    .toHaveAttribute("href", "/playground/");
  await expect(page.locator('[data-live-scene="node-graph"] .live-fallback img'))
    .toHaveAttribute("src", /node-graph-studio-dark\.png$/);
  const homeFrame = await liveFrame(page, "node-graph");
  await expect.poll(() => homeFrame.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene))
    .toBe("node-graph");
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => page.evaluate(() => ({
    viewport: innerWidth,
    document: document.documentElement.scrollWidth,
    liveRight: document.querySelector('[data-live-scene="node-graph"]').getBoundingClientRect().right,
  }))).toEqual({ viewport: 390, document: 390, liveRight: 374 });

  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto("/scenes/button.html");
  await expect(page.getByRole("heading", { name: "button", exact: true })).toBeVisible();
  await expect(page.locator(".themes img")).toHaveCount(2);
  await expect(page.locator("pre.code")).toContainText("Button");
  const sceneFrame = await liveFrame(page, "button");
  await expect.poll(() => sceneFrame.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene))
    .toBe("button");

  await page.goto("/playground/?backend=webgl");
  await page.waitForFunction(() => window.gpuiKitGalleryReady === true);
  await expect(page.locator("canvas")).toHaveAttribute("data-gpui-renderer", "webgl2");
  await expect.poll(() => page.evaluate(() => window.gpuiBoxConfig.mode)).toBe("playground");
  await page.waitForFunction(() =>
    JSON.parse(window.gpuiKitSemanticSnapshot).nodes
      .some(node => node.id === "browser.playground.title"),
  );
});
