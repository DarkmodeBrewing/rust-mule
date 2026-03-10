const page = document.documentElement.dataset.uiPage;

const pageModules = {
  overview: '/ui/assets/js/pages/overview.js',
  search: '/ui/assets/js/pages/search.js',
  'search-details': '/ui/assets/js/pages/search-details.js',
  'node-stats': '/ui/assets/js/pages/node-stats.js',
  logs: '/ui/assets/js/pages/logs.js',
  downloads: '/ui/assets/js/pages/downloads.js',
  settings: '/ui/assets/js/pages/settings.js',
};

const pageModule = pageModules[page];
if (!pageModule) {
  throw new Error(`Unknown UI page bootstrap target: ${page}`);
}

await import(pageModule);

if (!window.Alpine && !document.querySelector('script[data-ui-alpine="true"]')) {
  const script = document.createElement('script');
  script.defer = true;
  script.src = '/ui/assets/js/alpine.min.js';
  script.dataset.uiAlpine = 'true';
  document.head.appendChild(script);
}
