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
    ".section .panel, .section .shot, .section .creation, .section .chips, .section .rune, " +
    ".section .table-wrap, .section .doc, .section .aside, .section .tab, " +
    ".section .faq, .strip-inner, " +
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
      // Preload margin: reveal sections just before they enter the viewport
      // so fast scrolls and anchor jumps never land on a blank gap.
      { rootMargin: "0px 0px 400px 0px", threshold: 0 }
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

  /* ---------- creations gallery ---------- */

  var grid = $("#creations-grid");
  var count = $("#creations-count");
  var winnersOnly = $("#winners-only");
  var filters = $$(".creation-filter");
  var activeFilter = "all";

  var creations = function () {
    return $$(".creation", grid || document);
  };

  var updateCount = function () {
    if (!count || !grid) return;
    var visible = creations().filter(function (node) {
      return !node.hidden;
    }).length;
    var total = creations().length;
    count.textContent = visible + " of " + total + " shown";
  };

  var applyFilters = function () {
    if (!grid) return;
    creations().forEach(function (node) {
      var matchRun = activeFilter === "all" || node.dataset.run === activeFilter;
      var matchWinner = !winnersOnly || !winnersOnly.checked || node.dataset.winner === "1";
      node.hidden = !(matchRun && matchWinner);
    });
    updateCount();
  };

  if (grid) {
    filters.forEach(function (button) {
      button.addEventListener("click", function () {
        activeFilter = button.dataset.filter;
        filters.forEach(function (other) {
          var selected = other === button;
          other.classList.toggle("is-active", selected);
          other.setAttribute("aria-pressed", selected ? "true" : "false");
        });
        applyFilters();
      });
    });

    if (winnersOnly) {
      winnersOnly.addEventListener("change", applyFilters);
    }

    /* ---------- lightbox ---------- */

    var dialog = $("#creation-lightbox");
    var dialogImg = $("#creation-lightbox-img");
    var dialogCaption = $("#creation-lightbox-caption");
    var dialogPosition = $("#creation-position");
    var prevButton = $("#creation-prev");
    var nextButton = $("#creation-next");
    var closeButton = $("#creation-close");
    var current = 0;

    var visibleCreations = function () {
      return creations().filter(function (node) {
        return !node.hidden;
      });
    };

    var show = function (index) {
      var list = visibleCreations();
      if (list.length === 0) return;
      current = (index + list.length) % list.length;
      var node = list[current];
      var img = node.querySelector("img");
      if (dialogImg && img) {
        dialogImg.src = img.src;
        dialogImg.alt = img.alt;
      }
      if (dialogCaption) dialogCaption.textContent = node.dataset.caption || "";
      if (dialogPosition) dialogPosition.textContent = current + 1 + " / " + list.length;
    };

    grid.addEventListener("click", function (event) {
      var button = event.target.closest(".creation-open");
      if (!button || !dialog) return;
      var node = button.closest(".creation");
      var list = visibleCreations();
      show(list.indexOf(node));
      if (typeof dialog.showModal === "function") {
        dialog.showModal();
      }
    });

    if (prevButton) prevButton.addEventListener("click", function () { show(current - 1); });
    if (nextButton) nextButton.addEventListener("click", function () { show(current + 1); });
    if (closeButton) closeButton.addEventListener("click", function () { if (dialog) dialog.close(); });
    if (dialog) {
      dialog.addEventListener("click", function (event) {
        if (event.target === dialog) dialog.close();
      });
      dialog.addEventListener("keydown", function (event) {
        if (event.key === "ArrowLeft") show(current - 1);
        if (event.key === "ArrowRight") show(current + 1);
      });
    }

    applyFilters();
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

  /* ---------- channel tracklist ---------- */

  var tracklist = $(".tracklist ul");
  if (tracklist && "fetch" in window) {
    window.fetch("videos.json").then(function (response) {
      if (!response.ok) throw new Error("tracklist unavailable");
      return response.json();
    }).then(function (videos) {
      if (!videos || !videos.length) return;
      tracklist.innerHTML = "";
      videos.forEach(function (video) {
        var item = document.createElement("li");
        var link = document.createElement("a");
        link.href = "https://www.youtube.com/watch?v=" + video.id;
        link.target = "_blank";
        link.rel = "noopener";
        link.textContent = video.title;
        var length = document.createElement("span");
        length.textContent = video.duration;
        item.appendChild(link);
        item.appendChild(length);
        tracklist.appendChild(item);
      });
    }).catch(function () {
      /* keep the static fallback list */
    });
  }
})();
