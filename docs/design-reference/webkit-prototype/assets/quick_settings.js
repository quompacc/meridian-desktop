(() => {
  "use strict";

  let requestSequence = 0;
  let audioAvailable = false;
  let audioMuted = false;
  let powerProfile = null;
  let networkSnapshot = null;
  let selectedSsid = null;

  function post(capability, payload = {}) {
    const handler = window.webkit?.messageHandlers?.meridian;
    if (!handler) {
      return;
    }
    requestSequence += 1;
    handler.postMessage(JSON.stringify({
      version: 1,
      capability,
      request_id: `quick-settings-${requestSequence}`,
      ...payload,
    }));
  }

  function close() {
    post("quick-settings.close");
  }

  function showSystemView() {
    selectedSsid = null;
    const password = document.querySelector("[data-network-password]");
    password.value = "";
    document.querySelector("[data-network-password-form]").hidden = true;
    document.querySelector("[data-network-detail-view]").hidden = true;
    document.querySelector("[data-network-back]").hidden = true;
    document.querySelector("[data-settings-button]").hidden = false;
    document.querySelector("[data-view-title]").textContent = "System";
    document.querySelector("[data-quick-settings]").dataset.view = "system";
  }

  function showNetworkView() {
    document.querySelector("[data-network-detail-view]").hidden = false;
    document.querySelector("[data-network-back]").hidden = false;
    document.querySelector("[data-settings-button]").hidden = true;
    document.querySelector("[data-view-title]").textContent = "Netzwerk";
    document.querySelector("[data-quick-settings]").dataset.view = "network";
    renderNetworkDetails();
  }

  function renderNetworkDetails() {
    if (!networkSnapshot) {
      return;
    }
    const current = document.querySelector("[data-network-current]");
    const detail = document.querySelector("[data-network-current-detail]");
    current.textContent = networkSnapshot.connected
      ? (networkSnapshot.name || "Verbunden")
      : networkSnapshot.available ? "Nicht verbunden" : "Nicht verfügbar";
    detail.textContent = networkSnapshot.kind || "Netzwerk";

    const list = document.querySelector("[data-network-list]");
    list.replaceChildren();
    const networks = networkSnapshot.wifi_networks || [];
    for (const network of networks) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "network-row";
      row.classList.toggle("is-active", network.in_use);
      row.dataset.ssid = network.ssid;
      row.setAttribute("role", "listitem");

      const name = document.createElement("span");
      name.textContent = `${network.in_use ? "✓ " : ""}${network.ssid}${network.secured ? " · Geschützt" : ""}`;
      const meta = document.createElement("span");
      meta.className = "network-meta";
      meta.textContent = `${network.signal_percent} %`;
      row.append(name, meta);
      row.addEventListener("click", () => selectNetwork(network));
      list.append(row);
    }
    document.querySelector("[data-network-empty]").hidden = networks.length !== 0;
    document.querySelector("[data-network-disconnect]").hidden =
      !(networkSnapshot.connected && networkSnapshot.kind === "WLAN");
  }

  function selectNetwork(network) {
    if (network.in_use) {
      return;
    }
    if (network.secured && !network.known) {
      selectedSsid = network.ssid;
      document.querySelector("[data-network-password-label]").textContent =
        `Passwort für ${network.ssid}`;
      document.querySelector("[data-network-password-form]").hidden = false;
      document.querySelector("[data-network-password]").focus();
      return;
    }
    post("quick-settings.network.connect", { ssid: network.ssid });
    document.querySelector("[data-network-current]").textContent =
      `Verbinde mit ${network.ssid} …`;
  }

  function applyState(snapshot) {
    const network = snapshot.network;
    networkSnapshot = network;
    document.querySelector("[data-network-kind]").textContent =
      network.kind || "Netzwerk";
    document.querySelector("[data-network-detail]").textContent =
      !network.available
        ? "Nicht verfügbar"
        : network.connected
          ? (network.name || "Verbunden")
          : "Getrennt";
    document.querySelector("[data-network-tile]").classList.toggle(
      "is-active",
      network.connected,
    );
    if (document.querySelector("[data-quick-settings]").dataset.view === "network") {
      renderNetworkDetails();
    }

    const audio = snapshot.audio;
    const volume = audio.volume_percent ?? 0;
    audioAvailable = audio.available;
    audioMuted = audio.muted;
    document.querySelector("[data-audio-output]").textContent =
      audio.output_name || "Nicht verfügbar";
    document.querySelector("[data-volume]").textContent = !audio.available
      ? "—"
      : audio.muted
        ? "Stumm"
        : `${volume} %`;
    const slider = document.querySelector("[data-volume-control]");
    slider.value = volume;
    slider.disabled = !audio.available || audio.volume_percent === null;
    const mute = document.querySelector("[data-audio-mute]");
    mute.disabled = !audio.available;
    mute.classList.toggle("is-muted", audio.muted);
    mute.setAttribute("aria-pressed", String(audio.muted));

    const battery = snapshot.battery;
    document.querySelector("[data-battery]").textContent = !battery.present
      ? "Kein Akku"
      : `${battery.capacity} %${battery.charging ? " · Lädt" : ""}`;
    const powerLabels = {
      Eco: "eco",
      Standard: "standard",
      "Volle Leistung": "performance",
    };
    powerProfile = powerLabels[snapshot.power_profile] || null;
    document.querySelector("[data-power-profile]").textContent =
      snapshot.power_profile || "Nicht verfügbar";
    document.querySelector("[data-power-tile]").disabled = powerProfile === null;
  }

  window.meridianQuickSettings = { applyState };

  document.querySelector("[data-settings-button]").addEventListener("click", () => {
    post("quick-settings.open-settings");
  });
  document.querySelector("[data-network-tile]").addEventListener("click", showNetworkView);
  document.querySelector("[data-network-back]").addEventListener("click", showSystemView);
  document.querySelector("[data-network-refresh]").addEventListener("click", () => {
    post("quick-settings.network.refresh");
  });
  document.querySelector("[data-network-disconnect]").addEventListener("click", () => {
    post("quick-settings.network.disconnect");
    document.querySelector("[data-network-current]").textContent = "WLAN wird getrennt …";
  });
  document.querySelector("[data-network-password-cancel]").addEventListener("click", () => {
    selectedSsid = null;
    document.querySelector("[data-network-password]").value = "";
    document.querySelector("[data-network-password-form]").hidden = true;
  });
  document.querySelector("[data-network-password-form]").addEventListener("submit", (event) => {
    event.preventDefault();
    const input = document.querySelector("[data-network-password]");
    const password = input.value;
    input.value = "";
    document.querySelector("[data-network-password-form]").hidden = true;
    if (selectedSsid && password) {
      post("quick-settings.network.connect", { ssid: selectedSsid, password });
      document.querySelector("[data-network-current]").textContent =
        `Verbinde mit ${selectedSsid} …`;
    }
    selectedSsid = null;
  });

  const volumeControl = document.querySelector("[data-volume-control]");
  volumeControl.addEventListener("input", () => {
    document.querySelector("[data-volume]").textContent = `${volumeControl.value} %`;
  });
  volumeControl.addEventListener("change", () => {
    if (audioAvailable) {
      post("quick-settings.audio.set-volume", {
        volume_percent: Number(volumeControl.value),
      });
    }
  });

  document.querySelector("[data-audio-mute]").addEventListener("click", () => {
    if (audioAvailable) {
      audioMuted = !audioMuted;
      post("quick-settings.audio.toggle-mute");
    }
  });

  document.querySelector("[data-power-tile]").addEventListener("click", () => {
    const profiles = ["eco", "standard", "performance"];
    const current = profiles.indexOf(powerProfile);
    if (current >= 0) {
      post("quick-settings.power.set-profile", {
        power_profile: profiles[(current + 1) % profiles.length],
      });
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      if (document.querySelector("[data-quick-settings]").dataset.view === "network") {
        showSystemView();
      } else {
        close();
      }
    }
  });
})();
