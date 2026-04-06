/* ============================================================
   Umami — Tracking des téléchargements de cartes
   ============================================================ */
(function () {
  function slugify(text) {
    return text
      .toLowerCase()
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/(^-|-$)/g, "");
  }

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

  instrumentDownloads();

  if (typeof document$ !== "undefined") {
    document$.subscribe(function () {
      instrumentDownloads();
    });
  }
})();
