import { expect, test } from "@playwright/test";

async function openScene(page, testInfo, scene) {
  const { backend, renderer } = testInfo.project.metadata;
  await page.goto(`/index.html?scene=${scene}&theme=studio-light&backend=${backend}`);
  await page.waitForFunction(() => window.gpuiKitGalleryReady === true);
  await expect(page.locator("canvas")).toHaveCount(1);
  await expect(page.locator("canvas")).toHaveAttribute("data-gpui-renderer", renderer);
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

async function pointer(page, type, position, buttons) {
  await page.locator("canvas").dispatchEvent(type, {
    bubbles: true,
    button: 0,
    buttons,
    clientX: position.x,
    clientY: position.y,
    pointerId: 1,
    pointerType: "mouse",
  });
}

async function settle(page) {
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
}

test("ordinary control uses the catalog component and stable semantics", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "button");
  const { renderer } = testInfo.project.metadata;
  const primary = await node(page, "scene.button.primary");
  const canvas = page.locator("canvas");
  const idle = await canvas.screenshot();
  await pointer(page, "pointermove", center(primary), 0);
  await settle(page);
  await expect(canvas).toHaveCSS("cursor", "pointer");
  const hovered = await canvas.screenshot();
  if (renderer === "webgl2") {
    expect(hovered.equals(idle)).toBe(false);
  }
  await pointer(page, "pointerdown", center(primary), 1);
  await settle(page);
  const pressed = await canvas.screenshot();
  if (renderer === "webgl2") {
    expect(pressed.equals(hovered)).toBe(false);
  }
  await pointer(page, "pointerup", center(primary), 0);
  await settle(page);
});

test("text input accepts real browser keyboard input", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "input");
  const email = await node(page, "scene.input.invalid");
  await pointer(page, "pointerdown", center(email), 1);
  await pointer(page, "pointerup", center(email), 0);
  await page.keyboard.press("Control+A");
  await page.keyboard.type("edited@example.com");
  await expect.poll(async () => (await node(page, "scene.input.invalid")).value)
    .toBe("edited@example.com");
});

test("password scene edits and reveals without weakening redaction", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "auth-sign-in");
  const password = await node(page, "scene.auth.sign-in.password");
  await pointer(page, "pointerdown", center(password), 1);
  await pointer(page, "pointerup", center(password), 0);
  await page.keyboard.type("browser-password-needle");
  await expect.poll(async () => (await node(page, "scene.auth.sign-in.password")).value)
    .toBe("[REDACTED]");
  expect(JSON.stringify(await snapshot(page))).not.toContain("browser-password-needle");
  expect(await page.locator("[data-gpui-accessibility]").evaluate(element => element.outerHTML))
    .not.toContain("browser-password-needle");

  const reveal = page.locator('[data-gpui-accessibility] button[aria-label="Reveal password"]');
  await expect(reveal).toHaveCount(1);
  await reveal.evaluate(element => element.click());
  await settle(page);
  expect(JSON.stringify(await snapshot(page))).not.toContain("browser-password-needle");
  expect(await page.locator("[data-gpui-accessibility]").evaluate(element => element.outerHTML))
    .not.toContain("browser-password-needle");
});

test("verification scene edits one redacted segmented input", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "auth-verification");
  const code = await node(page, "scene.auth.verification.code");
  await pointer(page, "pointerdown", center(code), 1);
  await pointer(page, "pointerup", center(code), 0);
  await page.keyboard.type("ABCDEF");
  await expect.poll(async () => (await node(page, "scene.auth.verification.code")).description)
    .toBe("6/6");
  await expect.poll(async () => (await node(page, "scene.auth.verification.code")).value)
    .toBe("[REDACTED]");
  expect(JSON.stringify(await snapshot(page))).not.toContain("ABCDEF");
  expect(await page.locator("[data-gpui-accessibility]").evaluate(element => element.outerHTML))
    .not.toContain("ABCDEF");
});

test("overlay action dismisses the catalog dialog", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "dialog");
  const dialog = page.locator('[data-gpui-accessibility] [role="dialog"]');
  await expect(dialog).toHaveCount(1);
  const cancel = dialog.locator('button[aria-label="Cancel"]');
  await expect(cancel).toHaveCount(1);
  await cancel.evaluate(element => element.click());
  await expect(dialog).toHaveCount(0);
});

test("canvas drag follows the browser pointer path", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "node-graph");
  const before = await node(page, "scene.graph.validate");
  const from = center(before);
  await pointer(page, "pointermove", from, 0);
  await pointer(page, "pointerdown", from, 1);
  for (let step = 1; step <= 6; step += 1) {
    await pointer(page, "pointermove", {
      x: from.x + step * 8,
      y: from.y + step * (32 / 6),
    }, 1);
  }
  await pointer(page, "pointerup", { x: from.x + 48, y: from.y + 32 }, 0);
  await expect.poll(async () => (await node(page, "scene.graph.validate")).bounds.x)
    .toBeGreaterThan(before.bounds.x + 20);
});

test("developer data scenes render from the runtime catalog with stable semantics", async ({ page }, testInfo) => {
  const scenes = [
    ["log-stream", ["scene.log", "scene.log.entries", "scene.log.stale"]],
    ["diff-view", ["scene.diff.unified", "scene.diff.split"]],
    ["sparkline", ["scene.sparkline.rate", "scene.sparkline.stale"]],
  ];

  for (const [scene, ids] of scenes) {
    await openScene(page, testInfo, scene);
    const catalog = JSON.parse(await page.evaluate(() => window.gpuiKitCatalog));
    expect(catalog.scenes).toContain(scene);
    for (const id of ids) {
      expect(await node(page, id)).toBeDefined();
    }
  }
});

test("treegrid mirrors accessible ancestry and accepts row selection", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "tree-grid");
  const treegrid = page.locator('[data-gpui-accessibility] [role="treegrid"]');
  const row = treegrid.locator('[role="row"][aria-label="components"]');
  const cells = row.locator('[role="gridcell"]');
  await expect(treegrid).toHaveCount(1);
  await expect(row).toHaveCount(1);
  await expect(cells).toHaveCount(3);

  const docs = await node(page, "scene.tree-grid.files.docs");
  await pointer(page, "pointerdown", center(docs), 1);
  await pointer(page, "pointerup", center(docs), 0);
  await expect.poll(async () => (await node(page, "scene.tree-grid.files.docs")).selected)
    .toBe(false);
});

for (const [scene, id, value] of [
  ["cascader", "scene.cascader", "Release notes"],
  ["anchor-list", "scene.anchor-list.inputs", undefined],
  ["diagnostics-list", "scene.diagnostics.list.fixture-error", undefined],
]) {
  test(`common application scene ${scene} publishes catalog semantics`, async ({ page }, testInfo) => {
    await openScene(page, testInfo, scene);
    const target = await node(page, id);
    expect(target.bounds.width).toBeGreaterThan(0);
    expect(target.bounds.height).toBeGreaterThan(0);
    if (value !== undefined) expect(target.value).toBe(value);
  });
}

test("AccessKit DOM mirrors role, focus, action, and canvas-scaled bounds", async ({ page }, testInfo) => {
  await openScene(page, testInfo, "button");
  const primary = await node(page, "scene.button.primary");
  const canvas = await page.locator("canvas").boundingBox();
  const accessible = page.locator('[data-gpui-accessibility] button[aria-label="Primary"]');
  await expect(accessible).toHaveCount(1);
  await accessible.focus();
  await expect(accessible).toBeFocused();
  const bounds = await accessible.boundingBox();
  expect(Math.abs(bounds.x - (canvas.x + primary.bounds.x))).toBeLessThanOrEqual(1);
  expect(Math.abs(bounds.y - (canvas.y + primary.bounds.y))).toBeLessThanOrEqual(1);
  expect(Math.abs(bounds.width - primary.bounds.width)).toBeLessThanOrEqual(2);
  expect(Math.abs(bounds.height - primary.bounds.height)).toBeLessThanOrEqual(2);
});
