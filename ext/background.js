// Default API configuration fallback
const DEFAULT_API_URL = "http://localhost:5173/api/aria2/add/uris";

// Listen for new downloads
browser.downloads.onCreated.addListener(async (downloadItem) => {
  if (!downloadItem.url || downloadItem.url.startsWith('blob:')) return;
  // Immediately pause the download to intercept it
  try {
    await browser.downloads.pause(downloadItem.id);
  } catch (error) {
    console.error("Failed to pause download:", error);
    return;
  }

  // Find the active tab to display the interception prompt modal
  const tabs = await browser.tabs.query({ active: true, currentWindow: true });
  if (tabs.length > 0) {
    browser.tabs.sendMessage(tabs[0].id, {
      action: "INTERCEPT_DOWNLOAD",
      downloadId: downloadItem.id,
      url: downloadItem.url,
      filename: downloadItem.filename ? downloadItem.filename.split(/[/\\]/).pop() : "Unknown File"
    }).catch((err) => {
      // Fallback: If the page doesn't allow content scripts (e.g., about:addons), resume natively
      console.warn("Content script not accessible on this page. Resuming natively.", err);
      browser.downloads.resume(downloadItem.id);
    });
  } else {
    // No active tab visible, resume download via Firefox
    browser.downloads.resume(downloadItem.id);
  }
});

// Handle user selections from the content script modal
browser.runtime.onMessage.addListener(async (message) => {
  if (message.action === "RESOLVE_DOWNLOAD") {
    if (message.choice === "qateel") {
      // Cancel the native Firefox download session
      await browser.downloads.cancel(message.downloadId);
			await browser.downloads.erase({ id: message.downloadId });
      // Route the URL to Qateel backend
      sendToQateel(message.url);
    } else if (message.choice === "firefox") {
      await browser.downloads.resume(message.downloadId);

		} else if (message.choice === "cancel") {
      // User selected Firefox native download
			await browser.downloads.cancel(message.downloadId);
      await browser.downloads.erase({ id: message.downloadId });
    }
  }
});

// Helper function to send the data payload to route
async function sendToQateel(downloadUrl) {
  const settings = await browser.storage.local.get({ qateelAddress: DEFAULT_API_URL });

  try {
    const response = await fetch(settings.qateelAddress, {
      method: "POST",
      headers: {
        "Content-Type": "application/json"
      },
      body: JSON.stringify({
        uris: [downloadUrl],
        options: {}
      })
    });

    if (!response.ok) {
      throw new Error(`Server responded with status ${response.status}`);
    }
    console.log("Successfully sent URL to Qateel:", downloadUrl);
  } catch (error) {
    console.error("Failed to send download link to Qateel:", error);
    // Optional: You could trigger a native browser notification here to alert the user of failure
  }
}
