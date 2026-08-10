import { expect, test } from "@playwright/test";

async function openScene(page, scene) {
  await page.goto(`/index.html?scene=${scene}&theme=studio-light`);
  await page.waitForFunction(() => window.gpuiKitGalleryReady === true);
  await expect(page.locator("canvas")).toHaveCount(1);
}

async function snapshot(page) {
  return JSON.parse(await page.evaluate(() => window.gpuiKitSemanticSnapshot));
}

async function node(page, id) {
  await page.waitForFunction(
    expected => JSON.parse(window.gpuiKitSemanticSnapshot).nodes.some(node => node.id === expected),
    id,
  );
  return (await snapshot(page)).nodes.find(node => node.id === id);
}

function center(node) {
  return {
    x: node.bounds.x + node.bounds.width / 2,
    y: node.bounds.y + node.bounds.height / 2,
  };
}

test("ordinary control uses the catalog component and stable semantics", async ({ page }) => {
  await openScene(page, "button");
  const primary = await node(page, "scene.button.primary");
  await page.mouse.move(center(primary).x, center(primary).y);
  await expect.poll(async () => (await node(page, "scene.button.primary")).hovered).toBe(true);
  await page.mouse.down();
  await expect.poll(async () => (await node(page, "scene.button.primary")).pressed).toBe(true);
  await page.mouse.up();
  await expect.poll(async () => (await node(page, "scene.button.primary")).pressed).toBe(false);
});

test("text input accepts real browser keyboard input", async ({ page }) => {
  await openScene(page, "input");
  const email = await node(page, "scene.input.invalid");
  await page.mouse.click(center(email).x, center(email).y);
  await page.keyboard.press("Control+A");
  await page.keyboard.type("edited@example.com");
  await expect.poll(async () => (await node(page, "scene.input.invalid")).value)
    .toBe("edited@example.com");
});

test("overlay action dismisses the catalog dialog", async ({ page }) => {
  await openScene(page, "dialog");
  const cancel = await node(page, "scene.dialog.replace.cancel");
  await page.mouse.click(center(cancel).x, center(cancel).y);
  await expect.poll(async () => (await snapshot(page)).nodes.some(node => node.id === "scene.dialog.replace"))
    .toBe(false);
});

test("canvas drag follows the browser pointer path", async ({ page }) => {
  await openScene(page, "node-graph");
  const before = await node(page, "scene.graph.validate");
  const from = center(before);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 48, from.y + 32, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await node(page, "scene.graph.validate")).bounds.x)
    .toBeGreaterThan(before.bounds.x + 20);
});
