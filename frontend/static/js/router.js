// Simple hash-based SPA router
const router = {
  routes: {},
  current: null,

  on(path, handler) {
    this.routes[path] = handler;
    return this;
  },

  match(hash) {
    const h = hash.replace(/^#/, '') || '/';
    // Check exact match first, then parameterized routes
    if (this.routes[h]) return { handler: this.routes[h], params: {} };
    for (const [pattern, handler] of Object.entries(this.routes)) {
      const regex = new RegExp('^' + pattern.replace(/:\w+/g, '([^/]+)') + '$');
      const match = h.match(regex);
      if (match) {
        const keys = [...pattern.matchAll(/:(\w+)/g)].map(m => m[1]);
        const params = {};
        keys.forEach((k, i) => params[k] = match[i + 1]);
        return { handler, params };
      }
    }
    return null;
  },

  navigate(path) {
    window.location.hash = path;
  },

  start() {
    const self = this;
    window.addEventListener('hashchange', () => self._render());
    // Highlight active nav link
    window.addEventListener('hashchange', () => {
      document.querySelectorAll('.sidebar a').forEach(a => a.classList.remove('active'));
      const link = document.querySelector(`.sidebar a[href="${window.location.hash || '#/'}"]`);
      if (link) link.classList.add('active');
    });
    if (!window.location.hash) window.location.hash = '#/';
    this._render();
  },

  _render() {
    const hash = window.location.hash || '#/';
    const main = document.getElementById('main-content');
    const result = this.match(hash);
    if (result) {
      this.current = result;
      result.handler(main, result.params);
    } else {
      main.innerHTML = '<div class="page"><h1>404</h1><p>Page not found</p></div>';
    }
  }
};
