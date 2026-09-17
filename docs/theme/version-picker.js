// Version picker for the published documentation site.
//
// The site is laid out by `docs/scripts/build-versioned.sh` as:
//
//   /            the newest release
//   /dev/        the current main
//   /vX.Y.Z/     each release
//
// This reads `versions.json` from the site root rather than hard-coding a
// list, so a new release needs no edit here to appear — the tags are the
// source of truth in both places.
//
// It does nothing when `versions.json` is absent, which is the case for a
// local `mdbook serve`. A picker that broke local preview to serve the
// published site would be a bad trade.
(function () {
  "use strict";

  // Path segment naming a version, if the reader is inside one.
  var parts = window.location.pathname.split("/").filter(Boolean);
  var current = null;
  var depth = 0;
  for (var i = 0; i < parts.length; i++) {
    if (/^v\d+\.\d+\.\d+$/.test(parts[i]) || parts[i] === "dev") {
      current = parts[i];
      depth = i + 1;
      break;
    }
  }
  // Everything above the version segment, e.g. "/lumina" on project Pages.
  var base = "/" + parts.slice(0, depth ? depth - 1 : 0).join("/");
  if (base !== "/") base += "/";

  fetch(base + "versions.json")
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (data) {
      if (!data || !data.versions) return;
      var latest = data.latest;
      var shown = current || latest;

      var bar = document.createElement("div");
      bar.className = "lumina-version-bar";

      // An old version is worth saying loudly. Somebody reading v0.3 docs for
      // a v0.5 install will be puzzled by advice that no longer applies, and
      // will usually blame the software rather than the page.
      if (current && current !== latest && current !== "dev") {
        bar.classList.add("is-old");
        bar.appendChild(document.createTextNode(
          "You are reading the docs for " + current + ". The current release is "
        ));
        var a = document.createElement("a");
        a.href = base;
        a.textContent = latest;
        bar.appendChild(a);
        bar.appendChild(document.createTextNode("."));
      } else if (current === "dev") {
        bar.classList.add("is-dev");
        bar.appendChild(document.createTextNode(
          "You are reading the development docs, which describe unreleased behaviour. "
        ));
        var b = document.createElement("a");
        b.href = base;
        b.textContent = "Latest release (" + latest + ")";
        bar.appendChild(b);
      } else {
        bar.appendChild(document.createTextNode("Version: "));
      }

      var select = document.createElement("select");
      select.setAttribute("aria-label", "Documentation version");
      data.versions.forEach(function (v) {
        var opt = document.createElement("option");
        opt.value = v;
        opt.textContent = v === latest ? v + " (latest)" : v;
        if (v === shown) opt.selected = true;
        select.appendChild(opt);
      });
      select.addEventListener("change", function () {
        // Land on the same page in the chosen version where one exists, and
        // on its index where it does not — a 404 is a worse answer than the
        // table of contents.
        var rest = parts.slice(depth).join("/");
        var target = select.value === latest ? base : base + select.value + "/";
        window.location.href = target + (rest || "");
      });
      bar.appendChild(select);

      document.body.insertBefore(bar, document.body.firstChild);
    })
    .catch(function () { /* local preview: no versions.json, no picker */ });
})();
