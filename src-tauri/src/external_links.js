// A citation's DOI and the repository link are the only addresses this interface offers,
// and both belong in the browser the reader already has. Under a webview, an anchor with
// target="_blank" is not reliably interceptable through navigation handling and can open a
// second uncontrolled window instead, replacing the workspace with a publisher's page.
//
// The listener captures, so it runs before anything the page attaches, and it leaves every
// in-page anchor alone. The capability this calls into is scoped to the two hosts the page
// links, so a URL the interface did not put there cannot be opened through it.
(() => {
  document.addEventListener(
    'click',
    (event) => {
      const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null;
      if (!anchor) return;

      let destination;
      try {
        destination = new URL(anchor.href, document.baseURI);
      } catch {
        return;
      }
      if (destination.protocol !== 'http:' && destination.protocol !== 'https:') return;

      event.preventDefault();
      window.__TAURI_INTERNALS__.invoke('plugin:opener|open_url', { url: destination.href });
    },
    true,
  );
})();
