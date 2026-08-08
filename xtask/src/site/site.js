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
