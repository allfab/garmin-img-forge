/* ============================================================
   LemonSqueezy — Loader lemon.js + réinitialisation SPA
   ============================================================ */
(function () {
  var lemonReady = false;

  function initLemon() {
    if (window.createLemonSqueezy) {
      window.createLemonSqueezy();
      lemonReady = true;
    }
  }

  function loadLemonJS() {
    if (document.querySelector('script[src*="lemon.js"]')) {
      initLemon();
      return;
    }

    var script = document.createElement("script");
    script.src = "https://app.lemonsqueezy.com/js/lemon.js";
    script.defer = true;
    script.onload = function () {
      initLemon();
    };
    document.head.appendChild(script);
  }

  /* Pulsation uniquement au premier chargement de la session */
  function pulseOnce() {
    if (sessionStorage.getItem("zn-donate-pulsed")) return;
    var fab = document.querySelector(".zn-donate-fab");
    if (fab) {
      fab.style.animation = "donate-pulse 2s ease-in-out 1s 3";
      sessionStorage.setItem("zn-donate-pulsed", "1");
    }
  }

  /* Chargement initial */
  loadLemonJS();

  /* Réinitialisation après chaque navigation SPA Zensical */
  if (typeof document$ !== "undefined") {
    document$.subscribe(function () {
      if (lemonReady) {
        initLemon();
      }
      pulseOnce();
    });
  } else {
    pulseOnce();
  }
})();
