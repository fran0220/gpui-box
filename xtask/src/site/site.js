// Filtering the component list. Every row already carries what it matches on,
// so this needs no index and no network, and it keeps working if the search
// payload never loads.
(function () {
  var filter = document.getElementById("filter");
  if (!filter) return;

  var rows = Array.prototype.slice.call(document.querySelectorAll(".row"));
  var empty = document.getElementById("empty");

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

  // `/` focuses the filter, which is the one shortcut worth having on a page
  // whose only job is finding a name.
  document.addEventListener("keydown", function (event) {
    if (event.key === "/" && document.activeElement !== filter) {
      event.preventDefault();
      filter.focus();
      filter.select();
    }
    if (event.key === "Escape" && document.activeElement === filter) {
      filter.value = "";
      apply();
      filter.blur();
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
