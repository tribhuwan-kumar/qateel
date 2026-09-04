const DEFAULT_API_URL = "http://localhost:5173/api/aria2/add/uris";

// Load configuration
function restoreOptions() {
  browser.storage.local.get({ qateelAddress: DEFAULT_API_URL }).then((items) => {
    document.getElementById("qateelAddress").value = items.qateelAddress;
  }, (error) => {
    console.error(`Error loading settings: ${error}`);
  });
}

// Save configuration
function saveOptions(e) {
  e.preventDefault();
  const addressValue = document.getElementById("qateelAddress").value.trim();

  browser.storage.local.set({
    qateelAddress: addressValue || DEFAULT_API_URL
  }).then(() => {
    const status = document.getElementById("status");
    status.textContent = "Settings saved successfully.";
    setTimeout(() => {
      status.textContent = "";
    }, 2500);
  });
}

document.addEventListener("DOMContentLoaded", restoreOptions);
document.getElementById("save").addEventListener("click", saveOptions);
