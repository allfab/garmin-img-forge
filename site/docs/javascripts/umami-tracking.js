/* ============================================================
   Umami — Tracking comportemental du site

   Deux modes de tracking selon le cas d'usage :
   - data-umami-event : attributs HTML, Umami intercepte les clics
     automatiquement (téléchargements, liens externes)
   - umami.track()    : appel JS programmatique pour les événements
     non-clic (scroll, copie, recherche)
   ============================================================ */
(function () {
  var siteHost = window.location.hostname;

  function slugify(text) {
    return text
      .toLowerCase()
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/(^-|-$)/g, "");
  }

  /* ── Téléchargements de cartes ────────────────────────────── */
  function instrumentDownloads() {
    var links = document.querySelectorAll("a.md-button");

    links.forEach(function (link) {
      if (link.dataset.umamiEvent) return;

      var text = link.textContent.trim();
      if (!/t[ée]l[ée]charger/i.test(text)) return;

      var card = link.closest(".card") || link.closest("li");
      if (!card) return;

      var heading =
        card.querySelector("h2, h3, h4, p:first-child, .card-title");
      var label = heading ? heading.textContent.trim() : "";

      if (!label) {
        var strong = card.querySelector("strong");
        label = strong ? strong.textContent.trim() : "carte-inconnue";
      }

      link.setAttribute("data-umami-event", "download-carte");
      link.setAttribute("data-umami-event-carte", slugify(label));
    });
  }

  /* ── Téléchargements de binaires (mpforge/imgforge) ──────── */
  function instrumentBinaryDownloads() {
    var links = document.querySelectorAll(
      "a[href*='github.com/allfab/garmin-img-forge'][href*='/releases/download/']"
    );

    links.forEach(function (link) {
      if (link.dataset.umamiEvent) return;

      var href = link.getAttribute("href");
      var url, filename;
      try {
        url = new URL(href, window.location.origin);
        filename = url.pathname.split("/").pop();
      } catch (e) {
        return;
      }

      if (!filename) return;

      var parts = url.pathname.split("/");
      var tagIdx = parts.indexOf("download");
      var version =
        tagIdx >= 0 && tagIdx + 1 < parts.length ? parts[tagIdx + 1] : "";

      link.setAttribute("data-umami-event", "download-binaire");
      link.setAttribute("data-umami-event-binaire", filename);
      if (version) {
        link.setAttribute("data-umami-event-version", version);
      }
    });
  }

  /* ── Liens externes (scopé au contenu principal) ─────────── */
  function instrumentExternalLinks() {
    var container = document.querySelector(".md-content") || document;
    var links = container.querySelectorAll("a[href^='http']");

    links.forEach(function (link) {
      if (link.dataset.umamiEvent) return;

      var url;
      try {
        url = new URL(link.getAttribute("href"));
      } catch (e) {
        return;
      }

      if (url.hostname === siteHost) return;

      link.setAttribute("data-umami-event", "clic-externe");
      link.setAttribute("data-umami-event-url", url.hostname + url.pathname);
    });
  }

  /* ── Copie de blocs de code ──────────────────────────────── */
  function instrumentCodeCopy() {
    document.querySelectorAll(".md-clipboard").forEach(function (btn) {
      if (btn.dataset.umamiTracked) return;
      btn.dataset.umamiTracked = "1";

      btn.addEventListener("click", function () {
        var codeBlock = btn.closest(".highlight");
        var title = codeBlock
          ? codeBlock.querySelector(".filename, .title")
          : null;
        var label = title ? title.textContent.trim() : "";

        if (!label) {
          var h1 = document.querySelector("h1");
          label = slugify(h1 ? h1.textContent.trim() : location.pathname);
        }

        if (typeof umami !== "undefined") {
          umami.track("copie-code", { page: location.pathname, bloc: label });
        }
      });
    });
  }

  /* ── Scroll depth ────────────────────────────────────────── */
  var scrollThresholds = { 25: false, 50: false, 75: false, 100: false };

  function resetScrollTracking() {
    scrollThresholds = { 25: false, 50: false, 75: false, 100: false };
  }

  function onScroll() {
    var scrollTop = window.scrollY || document.documentElement.scrollTop;
    var docHeight =
      document.documentElement.scrollHeight -
      document.documentElement.clientHeight;

    if (docHeight <= 0) return;

    var percent = Math.round((scrollTop / docHeight) * 100);

    [25, 50, 75, 100].forEach(function (threshold) {
      if (percent >= threshold && !scrollThresholds[threshold]) {
        scrollThresholds[threshold] = true;

        if (typeof umami !== "undefined") {
          umami.track("scroll-depth", {
            page: location.pathname,
            seuil: threshold + "%",
          });
        }
      }
    });
  }

  /* ── Boutons "Soutenir" (LemonSqueezy) ──────────────────── */
  function instrumentDonateButtons() {
    var buttons = document.querySelectorAll(
      ".zn-donate-fab, .zn-donate-inline"
    );

    buttons.forEach(function (btn) {
      if (btn.dataset.umamiTracked) return;
      btn.dataset.umamiTracked = "1";

      btn.addEventListener("click", function () {
        var variant = btn.classList.contains("zn-donate-fab")
          ? "fab"
          : "inline";

        if (typeof umami !== "undefined") {
          umami.track("clic-soutenir", {
            page: location.pathname,
            variante: variant,
          });
        }
      });
    });
  }

  /* ── Recherche site (input persistant, attaché une seule fois) */
  function instrumentSearch() {
    var searchInput = document.querySelector(".md-search__input");
    if (!searchInput || searchInput.dataset.umamiTracked) return;
    searchInput.dataset.umamiTracked = "1";

    var debounceTimer;
    searchInput.addEventListener("input", function () {
      clearTimeout(debounceTimer);
      var query = searchInput.value.trim();

      debounceTimer = setTimeout(function () {
        if (query.length < 3) return;

        if (typeof umami !== "undefined") {
          umami.track("recherche", { terme: query });
        }
      }, 1500);
    });
  }

  /* ── Orchestration ───────────────────────────────────────── */
  function instrumentPage() {
    instrumentDownloads();
    instrumentBinaryDownloads();
    instrumentExternalLinks();
    instrumentCodeCopy();
    instrumentDonateButtons();
  }

  instrumentPage();
  instrumentSearch();
  window.addEventListener("scroll", onScroll, { passive: true });

  if (typeof document$ !== "undefined") {
    document$.subscribe(function () {
      resetScrollTracking();
      instrumentPage();
    });
  }
})();
