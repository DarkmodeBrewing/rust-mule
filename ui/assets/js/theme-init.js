(function () {
  var THEME_KEY = 'ui_theme';
  var allowed = { dark: true, light: true, hc: true };
  var stored = '';
  try {
    stored = localStorage.getItem(THEME_KEY) || '';
  } catch (_err) {
    stored = '';
  }

  if (allowed[stored]) {
    document.documentElement.setAttribute('data-theme', stored);
  } else if (!document.documentElement.getAttribute('data-theme')) {
    document.documentElement.setAttribute('data-theme', 'dark');
  }
})();
