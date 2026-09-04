document.addEventListener('DOMContentLoaded', async () => {
  // Set extension version
  const manifest = browser.runtime.getManifest();
	console.log("Manifest version:", manifest);
  document.getElementById('ext-version').textContent = `v${manifest.version}`;

  // Handle Extension Toggle
  const toggle = document.getElementById('ext-toggle');
  const settings = await browser.storage.local.get({
			isEnabled: true,
			qateelAddress: "http://localhost:5173/api/aria2/add/uris"
	});

  toggle.checked = settings.isEnabled;
  toggle.addEventListener('change', (e) => {
    browser.storage.local.set({ isEnabled: e.target.checked });
  });

  // Setup WebSocket for Qateel Status
  const ui = {
    indicator: document.getElementById('ws-indicator'),
    ver: document.getElementById('srv-ver'),
    admin: document.getElementById('srv-admin'),
    aria2: document.getElementById('srv-aria2')
  };

  // Convert HTTP route to WS route dynamically based on settings
  const baseUrl = new URL(settings.qateelAddress).origin;
  const wsUrl = baseUrl.replace(/^http/, 'ws') + "/api/ws/status";

  const ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    ui.indicator.classList.add('online');
  };

  ws.onmessage = (event) => {
    try {
      const status = JSON.parse(event.data);
      ui.ver.textContent = status.version;
      ui.admin.textContent = status.adminExists ? "Yes" : "No";
      ui.aria2.textContent = status.aria2Alive ? "Yes" : "No";
    } catch (err) {
      console.error("Failed to parse WS data", err);
    }
  };

  ws.onclose = () => {
    ui.indicator.classList.remove('online');
    ui.ver.textContent = "--";
    ui.admin.textContent = "--";
    ui.aria2.textContent = "--";
  };
});
