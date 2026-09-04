browser.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "INTERCEPT_DOWNLOAD") {
    showQateelModal(request.downloadId, request.url, request.filename);
  }
});

function showQateelModal(downloadId, url, filename) {
  if (document.getElementById("qateel-intercept-root")) return;

  // Use a container to inject our styles cleanly
  const root = document.createElement("div");
  root.id = "qateel-intercept-root";

  // Injecting the theme and animations
  const style = document.createElement("style");
  style.textContent = `
    .qateel-theme {
      --font-sans: "Red Hat Text", sans-serif;
      --background: oklch(1 0 0);
      --foreground: oklch(0.145 0.008 326);
      --card: oklch(1 0 0);
      --primary: oklch(0.212 0.019 322.12);
      --primary-foreground: oklch(0.985 0 0);
      --secondary: oklch(0.96 0.003 325.6);
      --secondary-foreground: oklch(0.212 0.019 322.12);
      --muted: oklch(0.96 0.003 325.6);
      --muted-foreground: oklch(0.542 0.034 322.5);
      --destructive: oklch(0.577 0.245 27.325);
      --border: oklch(0.922 0.005 325.62);
      --radius: 0.8rem;
    }
    @media (prefers-color-scheme: dark) {
      .qateel-theme {
        --background: oklch(0.145 0.008 326);
        --foreground: oklch(0.985 0 0);
        --card: oklch(0.212 0.019 322.12);
        --primary: oklch(0.922 0.005 325.62);
        --primary-foreground: oklch(0.212 0.019 322.12);
        --secondary: oklch(0.263 0.024 320.12);
        --secondary-foreground: oklch(0.985 0 0);
        --muted: oklch(0.263 0.024 320.12);
        --muted-foreground: oklch(0.711 0.019 323.02);
        --destructive: oklch(0.704 0.191 22.216);
        --border: oklch(1 0 0 / 10%);
      }
    }

    #qateel-overlay {
      position: fixed; top: 0; left: 0; width: 100vw; height: 100vh;
      background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px);
      z-index: 2147483647; display: flex; justify-content: center; align-items: center;
      font-family: var(--font-sans); color: var(--foreground);
      animation: qateelFadeIn 0.2s ease-out;
    }
    #qateel-modal {
      background: var(--card); border: 1px solid var(--border);
      padding: 32px; border-radius: var(--radius); width: 480px; max-width: 90%;
      box-shadow: 0 20px 40px rgba(0,0,0,0.2);
      animation: qateelScaleUp 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    }
    #qateel-modal h2 { margin: 0 0 16px 0; font-size: 24px; font-weight: 700; }
    #qateel-modal .file-box {
      background: var(--muted); border: 1px solid var(--border);
      padding: 16px; border-radius: calc(var(--radius) * 0.8);
      margin-bottom: 24px; word-break: break-all;
    }
    #qateel-modal .file-name { font-weight: 600; font-size: 15px; margin-bottom: 4px; }
    #qateel-modal .file-url { font-size: 12px; color: var(--muted-foreground); }
    #qateel-actions { display: flex; justify-content: flex-end; gap: 12px; }
    .qateel-btn {
      padding: 12px 20px; border: none; border-radius: calc(var(--radius) * 0.6);
      cursor: pointer; font-size: 14px; font-weight: 600; transition: opacity 0.2s;
    }
    .qateel-btn:hover { opacity: 0.9; }
    .btn-primary { background: var(--primary); color: var(--primary-foreground); }
    .btn-secondary { background: var(--secondary); color: var(--secondary-foreground); border: 1px solid var(--border); }
    .btn-destructive { background: transparent; color: var(--destructive); }
    .btn-destructive:hover { background: var(--muted); }

    @keyframes qateelFadeIn { from { opacity: 0; } to { opacity: 1; } }
    @keyframes qateelScaleUp { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: scale(1); } }
  `;

  root.appendChild(style);

  const overlay = document.createElement("div");
  overlay.id = "qateel-overlay";
  overlay.className = "qateel-theme";

  overlay.innerHTML = `
    <div id="qateel-modal">
      <h2>Intercept Download</h2>
      <div class="file-box">
        <div class="file-name">${filename}</div>
        <div class="file-url">${url.length > 80 ? url.substring(0, 80) + '...' : url}</div>
      </div>
      <div id="qateel-actions">
        <button class="qateel-btn btn-destructive" id="btn-cancel">Cancel</button>
        <button class="qateel-btn btn-secondary" id="btn-firefox">Native</button>
        <button class="qateel-btn btn-primary" id="btn-qateel">Send to Qateel</button>
      </div>
    </div>
  `;

  root.appendChild(overlay);
  document.body.appendChild(root);

  document.getElementById("btn-cancel").addEventListener("click", () => resolve("cancel"));
  document.getElementById("btn-firefox").addEventListener("click", () => resolve("firefox"));
  document.getElementById("btn-qateel").addEventListener("click", () => resolve("qateel"));

  function resolve(choice) {
    root.remove();
    browser.runtime.sendMessage({ action: "RESOLVE_DOWNLOAD", choice, downloadId, url });
  }
}
