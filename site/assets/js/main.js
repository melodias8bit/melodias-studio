// melodias-studio site: copy buttons, reveal on scroll, back to top.
// No dependencies, no build step.

(function () {
  "use strict";

  document.documentElement.classList.add("js");

  var $ = function (selector, root) {
    return (root || document).querySelector(selector);
  };
  var $$ = function (selector, root) {
    return Array.prototype.slice.call((root || document).querySelectorAll(selector));
  };

  var reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- copy buttons ---------- */

  function legacyCopy(text) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.left = "-9999px";
    document.body.appendChild(area);
    area.select();
    var ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (err) {
      ok = false;
    }
    document.body.removeChild(area);
    return ok;
  }

  document.addEventListener("click", function (event) {
    var button = event.target.closest(".copy");
    if (!button) return;

    var source = document.getElementById(button.getAttribute("data-copy"));
    if (!source) return;

    var text = source.innerText.replace(/\s+$/, "");
    var done = function () {
      button.textContent = "copied";
      button.classList.add("done");
      window.setTimeout(function () {
        button.textContent = "copy";
        button.classList.remove("done");
      }, 1600);
    };

    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text).then(done, function () {
        if (legacyCopy(text)) done();
      });
    } else if (legacyCopy(text)) {
      done();
    }
  });

  /* ---------- reveal on scroll ---------- */

  var targets = $$(
    ".section .sec-head, .section .tree, .section .card, .section .stage, " +
    ".section .panel, .section .shot, .section .chips, .section .rune, " +
    ".section .table-wrap, .section .doc, .section .aside, .section .tab, " +
    ".section .stepper, .section .faq, .section .cta-inner, .strip-inner, " +
    ".footer-col"
  );

  if (reduced || !("IntersectionObserver" in window)) {
    targets.forEach(function (node) {
      node.classList.add("reveal", "in");
    });
  } else {
    var counts = new Map();

    targets.forEach(function (node) {
      node.classList.add("reveal");
      var parent = node.parentElement;
      var index = counts.get(parent) || 0;
      counts.set(parent, index + 1);
      node.style.setProperty("--d", (index % 4) * 70 + "ms");
    });

    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (!entry.isIntersecting) return;
          entry.target.classList.add("in");
          observer.unobserve(entry.target);
        });
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.08 }
    );

    targets.forEach(function (node) {
      observer.observe(node);
    });
  }

  /* ---------- back to top ---------- */

  var toTop = $("#to-top");
  if (toTop) {
    var toggle = function () {
      toTop.classList.toggle("show", window.scrollY > 700);
    };
    window.addEventListener("scroll", toggle, { passive: true });
    toggle();
    toTop.addEventListener("click", function () {
      window.scrollTo({ top: 0, behavior: reduced ? "auto" : "smooth" });
    });
  }

  /* ---------- run mode tabs ---------- */

  var tabs = $$('.tabs .tab');
  if (tabs.length > 0) {
    tabs.forEach(function (tab) {
      tab.addEventListener("click", function () {
        tabs.forEach(function (other) {
          var selected = other === tab;
          other.setAttribute("aria-selected", selected ? "true" : "false");
          var panel = document.getElementById(other.getAttribute("aria-controls"));
          if (panel) panel.hidden = !selected;
        });
      });
    });
  }

  /* ---------- GitHub stars ---------- */

  var stars = $("#stars");
  if (stars && "fetch" in window) {
    fetch("https://api.github.com/repos/melodias8bit/melodias-studio")
      .then(function (response) {
        if (!response.ok) throw new Error("stars");
        return response.json();
      })
      .then(function (data) {
        if (data && typeof data.stargazers_count === "number") {
          stars.textContent = "Star " + data.stargazers_count;
        }
      })
      .catch(function () {});
  }

  /* ---------- hero 3D ---------- */

  var heroCanvas = $("#hero-3d");
  if (heroCanvas) {
    var start3D = function () {
      import("./hero-3d.js")
        .then(function (module) {
          try {
            module.initHero3D(heroCanvas);
          } catch (err) {}
        })
        .catch(function () {});
    };
    if ("requestIdleCallback" in window) {
      window.requestIdleCallback(start3D);
    } else {
      start3D();
    }
  }

  /* ---------- ambient video ---------- */

  var background = $(".bg-video");
  if (background) {
    if (reduced) {
      background.removeAttribute("autoplay");
      background.pause();
    } else {
      var play = background.play();
      if (play && typeof play.catch === "function") play.catch(function () {});
    }

    document.addEventListener("visibilitychange", function () {
      if (document.hidden) {
        background.pause();
      } else if (!reduced) {
        var again = background.play();
        if (again && typeof again.catch === "function") again.catch(function () {});
      }
    });
  }
})();
