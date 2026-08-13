// Filtering the home catalog. Every card already carries what it matches on,
// so this needs no index and no network, and it keeps working if the search
// payload never loads.
(function () {
  function bindFilter(inputId, selector, emptyId) {
    var filter = document.getElementById(inputId);
    if (!filter) return;
    var rows = Array.prototype.slice.call(document.querySelectorAll(selector));
    var empty = document.getElementById(emptyId);

    function apply() {
      var words = filter.value.toLowerCase().split(/\s+/).filter(Boolean);
      var shown = 0;
      rows.forEach(function (row) {
        var haystack = row.getAttribute("data-search") || "";
        var match = words.every(function (word) {
          return haystack.indexOf(word) !== -1;
        });
        row.hidden = !match;
        if (match) shown++;
      });
      if (empty) empty.hidden = shown !== 0;
    }

    filter.addEventListener("input", apply);
    return filter;
  }

  var sceneFilter = bindFilter("scene-filter", "[data-scene]", "scenes-empty");
  var componentFilter = bindFilter("component-filter", "[data-component]", "components-empty");
  bindFilter("filter", ".row", "empty");

  document.addEventListener("keydown", function (event) {
    var target = document.activeElement;
    var filters = [sceneFilter, componentFilter].filter(Boolean);
    if (event.key === "/" && filters.indexOf(target) === -1) {
      event.preventDefault();
      var next = sceneFilter || componentFilter;
      if (next) {
        next.focus();
        next.select();
      }
    }
    if (event.key === "Escape" && filters.indexOf(target) !== -1) {
      target.value = "";
      target.dispatchEvent(new Event("input"));
      target.blur();
    }
  });
})();

// Compose keeps both themes of one scene on the home page. Changing the
// selected scene rewrites the two live embeds and the full-size link; the
// verified captures stay visible until each iframe reports its first frame.
(function () {
  var dark = document.querySelector('[data-live-theme="studio-dark"]');
  var light = document.querySelector('[data-live-theme="studio-light"]');
  var filter = document.getElementById("compose-filter");
  var full = document.getElementById("compose-full");
  if (!dark || !light) return;

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
    if (filter && filter.value !== scene) filter.value = scene;
    var url = new URL(location.href);
    url.searchParams.set("scene", scene);
    history.replaceState(null, "", url);
  }

  if (filter) {
    filter.addEventListener("change", function () {
      selectScene(filter.value.trim());
    });
    filter.addEventListener("keydown", function (event) {
      if (event.key === "Enter") {
        event.preventDefault();
        selectScene(filter.value.trim());
      }
    });
  }

  var requested = new URLSearchParams(location.search);
  var scene = requested.get("scene");
  var component = requested.get("component");
  if (scene) selectScene(scene);
  if (component) {
    var target = document.getElementById("component-" + component.toLowerCase().replace(/[^a-z0-9]+/g, "-"));
    if (target) target.scrollIntoView();
  }
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
