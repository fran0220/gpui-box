// Filtering the catalog. Every card and rail link already carries what it
// matches on, so this needs no index and no network.
(function () {
  function bindCatalogFilter() {
    var filter = document.getElementById("component-filter");
    if (!filter) return filter;
    var items = Array.prototype.slice.call(document.querySelectorAll("[data-component]"));
    var modules = Array.prototype.slice.call(document.querySelectorAll("[data-module]"));
    var empty = document.getElementById("components-empty");

    function apply() {
      var words = filter.value.toLowerCase().split(/\s+/).filter(Boolean);
      var shown = 0;
      items.forEach(function (item) {
        var haystack = item.getAttribute("data-search") || item.textContent.toLowerCase();
        var match = words.every(function (word) {
          return haystack.indexOf(word) !== -1;
        });
        item.hidden = !match;
        if (match && item.classList.contains("tile")) shown++;
      });
      modules.forEach(function (module) {
        module.hidden = module.querySelectorAll("[data-component]:not([hidden])").length === 0;
      });
      if (empty) empty.hidden = shown !== 0 || items.every(function (item) {
        return !item.classList.contains("tile");
      });
    }

    filter.addEventListener("input", apply);
    return filter;
  }

  var catalogFilter = bindCatalogFilter();
  var composeFilter = document.getElementById("compose-filter");

  document.addEventListener("keydown", function (event) {
    var target = document.activeElement;
    var filters = [catalogFilter, composeFilter].filter(Boolean);
    if (event.key === "/" && filters.indexOf(target) === -1 && !event.metaKey && !event.ctrlKey) {
      var next = catalogFilter || composeFilter;
      if (!next) return;
      event.preventDefault();
      next.focus();
      next.select();
    }
    if (event.key === "Escape" && filters.indexOf(target) !== -1) {
      target.value = "";
      target.dispatchEvent(new Event("input"));
      target.blur();
    }
  });
})();

// The home specimen keeps both themes of one scene. Changing the selected
// scene rewrites the two live embeds and the full-size link; the verified
// captures stay visible until each iframe reports its first frame. The
// query string is left alone so an old `?scene=` link can still redirect
// to compose.
(function () {
  var dark = document.querySelector('[data-live-theme="studio-dark"]');
  var light = document.querySelector('[data-live-theme="studio-light"]');
  var filter = document.getElementById("compose-filter");
  var full = document.getElementById("compose-full");
  if (!dark || !light || !filter) return;

  function setEmbed(host, scene, theme) {
    var fallback = host.querySelector(".live-fallback");
    var image = host.querySelector("img");
    var frame = host.querySelector("iframe");
    var href = "/compose/?scene=" + encodeURIComponent(scene) + "&theme=" + encodeURIComponent(theme);
    host.setAttribute("data-live-scene", scene);
    host.classList.remove("is-ready");
    if (fallback) fallback.setAttribute("href", href);
    if (image) {
      var current = image.getAttribute("src") || "";
      image.setAttribute("src", current.replace(/\/[^/]+-(studio-(?:dark|light))\.png$/, "/" + scene + "-" + theme + ".png"));
      image.setAttribute("alt", "The verified " + scene + " scene in " + theme);
    }
    if (frame) {
      frame.setAttribute("title", "Live GPUI Box " + scene + " scene in " + theme);
      frame.setAttribute("src", href + "&embed=1");
      frame.setAttribute("tabindex", "-1");
    }
  }

  function selectScene(scene) {
    if (!scene) return;
    setEmbed(dark, scene, "studio-dark");
    setEmbed(light, scene, "studio-light");
    if (full) full.setAttribute("href", "/compose/?scene=" + encodeURIComponent(scene) + "&theme=studio-dark");
    if (filter.value !== scene) filter.value = scene;
  }

  filter.addEventListener("change", function () {
    selectScene(filter.value.trim());
  });
  filter.addEventListener("keydown", function (event) {
    if (event.key === "Enter") {
      event.preventDefault();
      selectScene(filter.value.trim());
    }
  });
})();

// A live scene is an enhancement over its committed capture. Keep the static
// image visible until the embedded GPUI surface has rendered its first semantic
// frame; a renderer failure therefore leaves useful content rather than a
// blank rectangle.
(function () {
  function markReady(frame) {
    var host = frame.closest(".live-embed");
    if (!host) return;
    frame.removeAttribute("tabindex");
    host.classList.add("is-ready");
  }

  window.addEventListener("message", function (event) {
    if (event.origin !== window.location.origin) return;
    if (!event.data || event.data.type !== "gpui-box-ready") return;
    Array.prototype.forEach.call(document.querySelectorAll(".live-frame"), function (frame) {
      if (frame.contentWindow === event.source) markReady(frame);
    });
  });
})();

(function () {
  var key = "gpui-box-theme";
  var button = document.getElementById("theme-toggle");

  function current() {
    return document.documentElement.getAttribute("data-theme") || "studio-dark";
  }

  function apply(theme) {
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(key, theme);
    } catch (error) {}
    if (button) {
      var next = theme === "studio-light" ? "studio-dark" : "studio-light";
      button.setAttribute("aria-label", "Use " + next);
      button.textContent = theme === "studio-light" ? "Dark" : "Light";
    }
  }

  if (button) {
    button.addEventListener("click", function () {
      apply(current() === "studio-light" ? "studio-dark" : "studio-light");
    });
    apply(current());
  }
})();

(function () {
  function openHash() {
    var id = location.hash.replace(/^#/, "");
    if (!id) return;
    var target = document.getElementById(id);
    if (target && target.tagName === "DETAILS") target.open = true;
  }

  openHash();
  window.addEventListener("hashchange", openHash);
})();

(function () {
  Array.prototype.forEach.call(document.querySelectorAll("[data-copy]"), function (button) {
    button.addEventListener("click", function () {
      var block = button.closest(".copy-block");
      var code = block && block.querySelector("pre");
      if (!code || !navigator.clipboard) return;
      navigator.clipboard.writeText(code.innerText).then(function () {
        button.textContent = "Copied";
        window.setTimeout(function () {
          button.textContent = "Copy";
        }, 1200);
      });
    });
  });
})();
