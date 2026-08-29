import { expect, test } from "@playwright/test";

async function liveFrame(page, scene, theme = "studio-dark") {
  const host = page.locator(`[data-live-scene="${scene}"][data-live-theme="${theme}"]`);
  const iframe = host.locator("iframe");
  await expect(iframe).toHaveAttribute("loading", "lazy");
  await expect(iframe).toHaveAttribute("title", `Live GPUI Box ${scene} scene in ${theme}`);
  await expect(host).toHaveClass(/is-ready/, { timeout: 30_000 });
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
  await expect(page.getByRole("heading", { name: "Independent GPUI, and the kit that sits on it." }))
    .toBeVisible();
  await expect(page.getByRole("link", { name: "Components", exact: true }))
    .toHaveAttribute("href", "/components/");
  await expect(page.getByRole("link", { name: "Docs", exact: true }))
    .toHaveAttribute("href", "/docs/");
  await expect(page.locator('[data-live-scene="node-graph"][data-live-theme="studio-dark"] .live-fallback img'))
    .toHaveAttribute("src", /node-graph-studio-dark\.png$/);
  await expect(page.locator('[data-live-scene="node-graph"][data-live-theme="studio-light"] .live-fallback img'))
    .toHaveAttribute("src", /node-graph-studio-light\.png$/);
  const homeFrame = await liveFrame(page, "node-graph");
  await expect.poll(() => homeFrame.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene))
    .toBe("node-graph");
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => page.evaluate(() => ({
    viewport: innerWidth,
    document: document.documentElement.scrollWidth,
    liveRight: document.querySelector('[data-live-scene="node-graph"][data-live-theme="studio-dark"]').getBoundingClientRect().right,
  }))).toEqual({ viewport: 390, document: 390, liveRight: 374 });

  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto("/?scene=button#compose");
  await expect(page).toHaveURL(/\/compose\/\?scene=button/);

  await page.goto("/components/Button.html");
  await expect(page.getByRole("heading", { name: /Button/ })).toBeVisible();
  await expect(page.locator("details#construct")).not.toHaveAttribute("open");
  await expect(page.locator("details#construct")).toContainText("new(ident");

  await page.goto("/docs/");
  await expect(page.locator(".rail a").first()).toHaveText("MCP");
  await expect(page.locator(".rail a").first()).toHaveAttribute("href", "/mcp/");

  await page.goto("/compose/?backend=webgl");
  await page.waitForFunction(() => window.gpuiKitGalleryReady === true);
  await expect(page.locator("canvas")).toHaveAttribute("data-gpui-renderer", "webgl2");
  await expect.poll(() => page.evaluate(() => window.gpuiBoxConfig.mode)).toBe("playground");
  await page.waitForFunction(() =>
    JSON.parse(window.gpuiKitSemanticSnapshot).nodes
      .some(node => node.id === "browser.playground.title"),
  );
});
