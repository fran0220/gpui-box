import { chromium, expect, test } from "@playwright/test";

let pageErrors;
test.beforeEach(async ({ page }) => {
  pageErrors = [];
  page.on("pageerror", error => pageErrors.push(error.message));
});
test.afterEach(() => expect(pageErrors).toEqual([]));

async function open(page, scene = "button", mode = "playground") {
  await page.goto(`/index.html?scene=${scene}&mode=${mode}&backend=webgl`);
  // Construction precedes the first ResizeObserver delivery and surface size.
  await page.waitForFunction(() => window.gpuiKitGalleryReady === true
    && JSON.parse(window.gpuiBoxViewport).width > 0
    && document.querySelector("canvas").width > 1);
  await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxViewport).width)).toBe(390);
  expect(await page.evaluate(() => matchMedia("(pointer: coarse)").matches)).toBe(true);
}
async function node(page, id) {
  return page.evaluate(id => JSON.parse(window.gpuiKitSemanticSnapshot).nodes.find(n => n.id === id), id);
}
async function center(page, id) {
  // The catalog is virtualized: new exhibits can move a named row offscreen.
  // Wheel input only prepares the fixture; the assertions still use real touch.
  if (id.startsWith("browser.playground.scenes.")) {
    for (let attempt = 0; attempt < 10 && !(await node(page, id)); attempt++) {
      const list = await node(page, "browser.playground.scenes");
      const firstRow = () => page.evaluate(() => {
        const row = JSON.parse(window.gpuiKitSemanticSnapshot).nodes
          .find(n => n.id.startsWith("browser.playground.scenes."));
        return row && [row.id, row.bounds.y];
      });
      const before = await firstRow();
      await page.mouse.move(list.bounds.x + list.bounds.width / 2,
        list.bounds.y + list.bounds.height / 2);
      await page.mouse.wheel(0, list.bounds.height / 2);
      await expect.poll(firstRow).not.toEqual(before);
    }
  }
  await expect.poll(() => node(page, id)).toBeTruthy();
  const { bounds: b } = await node(page, id);
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}
async function tap(page, id) {
  const p = await center(page, id);
  await page.touchscreen.tap(p.x, p.y);
}

test("trusted touch selects a scene without opening the keyboard", async ({ page }) => {
  await open(page);
  await tap(page, "browser.playground.scenes.badge");
  await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("badge");
  await expect(page.locator("[data-gpui-input]")).not.toBeFocused();
  expect(await page.evaluate(() => innerWidth)).toBe(390);
});

test("touch focus and keyless text insertion reach the focused form", async ({ page }) => {
  await open(page);
  await tap(page, "browser.playground.search.query");
  const input = page.locator("[data-gpui-input]");
  await expect(input).toBeFocused();
  // CDP insertText produces trusted beforeinput/input without keydown, like
  // dictation/soft-keyboard insertion. It does not emulate an OS keyboard.
  await page.keyboard.insertText("accordion");
  await expect.poll(async () => (await node(page, "browser.playground.search.query")).value).toBe("accordion");
  await input.evaluate(el => el.dispatchEvent(new InputEvent("beforeinput", {
    inputType: "deleteContentBackward", cancelable: true, bubbles: true,
  })));
  await expect.poll(async () => (await node(page, "browser.playground.search.query")).value).toBe("accordio");
  await page.screenshot({ path: "../../.amp/in/artifacts/mobile-search.png" });
});

test("composition commits once and blur clears the composition session", async ({ page }) => {
  await open(page);
  await tap(page, "browser.playground.search.query");
  const input = page.locator("[data-gpui-input]");
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.imeSetComposition", { text: "你好", selectionStart: 2, selectionEnd: 2 });
  await page.keyboard.insertText("你好");
  await expect.poll(async () => (await node(page, "browser.playground.search.query")).value).toBe("你好");
  await input.evaluate(el => el.blur());
  await input.focus();
  await input.evaluate(el => el.dispatchEvent(new CompositionEvent("compositionend", { data: "stale" })));
  await page.keyboard.insertText("x");
  await expect.poll(async () => (await node(page, "browser.playground.search.query")).value).toBe("你好x");
});

test("backgrounding a composing form hides input without reentrant editing", async ({ page }) => {
  await open(page);
  await tap(page, "browser.playground.search.query");
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.imeSetComposition", { text: "草稿", selectionStart: 2, selectionEnd: 2 });
  await page.evaluate(() => dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  await expect(page.locator("[data-gpui-input]")).not.toBeFocused();
  await page.evaluate(() => dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await tap(page, "browser.playground.search.query");
  await page.keyboard.insertText("x");
  await expect.poll(async () => (await node(page, "browser.playground.search.query")).value).toContain("x");
});

test("trusted touch cancellation cannot activate a pending scene", async ({ page }) => {
  await open(page);
  const p = await center(page, "browser.playground.scenes.badge");
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ ...p, id: 7 }] });
  await cdp.send("Input.dispatchTouchEvent", { type: "touchCancel", touchPoints: [] });
  expect(await page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("button");
  await tap(page, "browser.playground.scenes.badge");
  await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("badge");
});

test("a moving second contact consumes the first pending tap", async ({ page }) => {
  await open(page);
  const cdp = await page.context().newCDPSession(page);
  const p = await center(page, "browser.playground.scenes.badge");
  const first = { ...p, id: 4 };
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [first] });
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [first, { x: p.x + 60, y: p.y + 20, id: 8 }] });
  await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [first, { x: p.x + 110, y: p.y + 20, id: 8 }] });
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  // Ignoring the non-primary contact incorrectly commits the stationary tap.
  expect(await page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("button");
  await tap(page, "browser.playground.scenes.badge");
  await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("badge");
});

test("pagehide cancels contact before persisted pageshow resumes", async ({ page }) => {
  await open(page);
  const p = await center(page, "browser.playground.scenes.badge");
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ ...p, id: 9 }] });
  // PageTransitionEvent is a deterministic lifecycle fixture, not evidence of
  // native app switching or a browser's BFCache admission policy.
  await page.evaluate(() => dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  expect(await page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("button");
  await page.evaluate(() => dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await tap(page, "browser.playground.scenes.badge");
  await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("badge");
});

test("safe areas and residual keyboard occlusion relayout, zoom is not IME", async ({ page }) => {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setSafeAreaInsetsOverride", { insets: { top: 23, right: 7, bottom: 31, left: 11 } });
  await open(page);
  const metrics = () => page.evaluate(() => JSON.parse(window.gpuiBoxViewport));
  await expect.poll(metrics).toMatchObject({ top: 23, right: 7, bottom: 31, left: 11 });
  expect((await node(page, "browser.playground.title")).bounds.y).toBeGreaterThanOrEqual(23);
  await tap(page, "browser.playground.search.query");
  await expect(page.locator("[data-gpui-input]")).toBeFocused();
  // Browser geometry fixture: emulation has no OS keyboard. Use asymmetric
  // offset/height so forgetting the visual viewport offset fails this test.
  await page.evaluate(() => {
    Object.defineProperties(visualViewport, {
      height: { configurable: true, value: 504 },
      offsetTop: { configurable: true, value: 40 },
    });
    visualViewport.dispatchEvent(new Event("resize"));
  });
  await expect.poll(metrics).toMatchObject({ top: 40, bottom: 300, height: 844 });
  await page.screenshot({ path: "../../.amp/in/artifacts/mobile-keyboard-insets.png" });
  await page.evaluate(() => {
    Object.defineProperty(visualViewport, "scale", { configurable: true, value: 2 });
    visualViewport.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(metrics).toMatchObject({ top: 23, bottom: 31 });
  await page.locator("[data-gpui-input]").evaluate(el => el.blur());
  await page.evaluate(() => {
    for (const key of ["height", "offsetTop", "scale"]) delete visualViewport[key];
    visualViewport.dispatchEvent(new Event("resize"));
  });
  await page.setViewportSize({ width: 844, height: 390 });
  await expect.poll(metrics).toMatchObject({ width: 844, height: 390, top: 23, bottom: 31 });
  await page.screenshot({ path: "../../.amp/in/artifacts/mobile-landscape.png" });
});

test("layout-resizing keyboard geometry is not counted twice", async ({ page }) => {
  await open(page);
  await tap(page, "browser.playground.search.query");
  await page.setViewportSize({ width: 390, height: 544 });
  await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxViewport)))
    .toMatchObject({ height: 544, bottom: 0, top: 0 });
  await expect(page.locator("[data-gpui-input]")).toBeFocused();
});

test("trusted touch pan scrolls the list rather than activating its row", async ({ page }) => {
  await open(page);
  const cdp = await page.context().newCDPSession(page);
  const p = await center(page, "browser.playground.scenes.card");
  const before = await node(page, "browser.playground.scenes.button");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ ...p, id: 3 }] });
  for (const dy of [15, 35, 65, 95]) {
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: p.x, y: p.y - dy, id: 3 }] });
  }
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await expect.poll(async () => (await node(page, "browser.playground.scenes.button"))?.bounds.y).not.toBe(before.bounds.y);
  expect(await page.evaluate(() => JSON.parse(window.gpuiBoxSelection).scene)).toBe("button");
});

for (const [observation, expectedBacking] of [["rounding", 781], ["contradictory", 780]]) {
  test(`CSS sizing preserves ${observation} device-pixel observations correctly`, async ({ page }) => {
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.addInitScript(observation => {
      const NativeObserver = ResizeObserver;
      window.ResizeObserver = class extends NativeObserver {
        constructor(callback) {
          super((entries, observer) => callback(entries.map(entry => {
            Object.defineProperty(entry, "devicePixelContentBoxSize", { value: [{
              inlineSize: observation === "rounding" ? entry.contentRect.width * devicePixelRatio + 1 : entry.contentRect.width,
              blockSize: entry.contentRect.height * devicePixelRatio,
            }] });
            return entry;
          }), observer));
        }
      };
    }, observation);
    await open(page, "button", "scene");
    await expect.poll(() => page.evaluate(() => {
      const canvas = document.querySelector("canvas");
      return { backing: canvas.width, css: canvas.getBoundingClientRect().width,
        logical: JSON.parse(window.gpuiBoxViewport).width };
    })).toEqual({ backing: expectedBacking, css: 390, logical: 390 });
    await page.locator("canvas").evaluate(el => { el.style.width = "389.75px"; });
    await expect.poll(() => page.evaluate(() => JSON.parse(window.gpuiBoxViewport).width)).toBe(389.75);
    expect(errors).toEqual([]);
  });
}

test("native Chromium scale 2 control has coherent CSS and backing dimensions", async () => {
  const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--force-device-scale-factor=2", "--window-size=920,1000"] });
  try {
    // No CDP viewport or deviceScaleFactor emulation: Chromium owns scale.
    const context = await browser.newContext({ viewport: null, deviceScaleFactor: undefined, isMobile: false });
    const page = await context.newPage();
    await page.goto("http://127.0.0.1:4173/?mode=playground&backend=webgl");
    await page.waitForFunction(() => window.gpuiKitGalleryReady && JSON.parse(window.gpuiBoxViewport).width > 0);
    await expect.poll(() => page.evaluate(() => {
      const canvas = document.querySelector("canvas");
      return { dpr: devicePixelRatio, physical: canvas.width, css: canvas.getBoundingClientRect().width,
        logical: JSON.parse(window.gpuiBoxViewport).width };
    })).toEqual({ dpr: 2, physical: 1840, css: 920, logical: 920 });
    await page.screenshot({ path: "../../.amp/in/artifacts/chromium-native-scale2.png" });
  } finally { await browser.close(); }
});
