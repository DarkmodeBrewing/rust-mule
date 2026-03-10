import '/ui/assets/js/app.js';

if (!window.Alpine && !document.querySelector('script[data-ui-alpine="true"]')) {
  const script = document.createElement('script');
  script.defer = true;
  script.src = '/ui/assets/js/alpine.min.js';
  script.dataset.uiAlpine = 'true';
  document.head.appendChild(script);
}
