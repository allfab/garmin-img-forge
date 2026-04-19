(function () {
  function init() {
    if (typeof mediumZoom !== 'function') return;
    mediumZoom('article img:not(.no-zoom):not([src*="favicon"])', {
      margin: 24,
      background: 'rgba(0, 0, 0, 0.85)',
      scrollOffset: 48,
    });
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
