"use strict";

const search = document.querySelector(".search input");
const heading = document.querySelector("#applications-title");
const headingSummary = document.querySelector(".section-heading p");
const launcher = document.querySelector("[data-launcher]");
const settingsView = document.querySelector("[data-settings-view]");
const categories = Array.from(document.querySelectorAll(".category"));
const applications = Array.from(document.querySelectorAll(".app"));
const settingsCategories = Array.from(document.querySelectorAll("[data-settings-category]"));
const settingsPages = Array.from(document.querySelectorAll("[data-settings-page]"));
const appearanceThemeButtons = Array.from(document.querySelectorAll("[data-appearance-theme]"));
const wallpaperModeButtons = Array.from(document.querySelectorAll("[data-wallpaper-mode]"));
const wallpaperName = document.querySelector("[data-wallpaper-name]");

let activeCategory = "favorites";
let bridgeRequestId = 0;
let settingsOpenedExternally = false;

function requestBridge(capability, payload = {}) {
  const handler = globalThis.webkit?.messageHandlers?.meridian;
  if (!handler) {
    return false;
  }
  bridgeRequestId += 1;
  handler.postMessage(JSON.stringify({
    ...payload,
    version: 1,
    capability,
    request_id: `launcher-${bridgeRequestId}`,
  }));
  return true;
}

function visibleApplications() {
  return applications.filter((application) => !application.hidden);
}

function selectApplication(application) {
  for (const candidate of applications) {
    candidate.classList.toggle("is-selected", candidate === application);
  }
}

function applyFilter() {
  const query = search.value.trim().toLocaleLowerCase();
  let visibleCount = 0;

  for (const application of applications) {
    const matchesCategory = activeCategory === "all"
      || (activeCategory === "favorites" && application.dataset.favorite === "true")
      || application.dataset.category === activeCategory;
    const matchesQuery = !query
      || application.dataset.search.toLocaleLowerCase().includes(query);
    application.hidden = !(matchesCategory && matchesQuery);
    if (!application.hidden) {
      visibleCount += 1;
    }
  }

  headingSummary.textContent = query
    ? `${visibleCount} Treffer`
    : `${visibleCount} Anwendungen`;

  const selected = document.querySelector(".app.is-selected");
  if (!selected || selected.hidden) {
    selectApplication(visibleApplications()[0] || null);
  }
}

function activateCategory(button) {
  activeCategory = button.dataset.category;
  for (const category of categories) {
    const selected = category === button;
    category.classList.toggle("is-selected", selected);
    if (selected) {
      category.setAttribute("aria-current", "page");
    } else {
      category.removeAttribute("aria-current");
    }
  }
  heading.textContent = button.dataset.title;
  applyFilter();
}

function focusRelative(items, current, offset) {
  const index = items.indexOf(current);
  const next = Math.max(0, Math.min(items.length - 1, index + offset));
  items[next]?.focus();
}

function selectSettingsCategory(button) {
  const category = button.dataset.settingsCategory;
  for (const candidate of settingsCategories) {
    const selected = candidate === button;
    candidate.classList.toggle("is-selected", selected);
    if (selected) {
      candidate.setAttribute("aria-current", "page");
    } else {
      candidate.removeAttribute("aria-current");
    }
  }
  for (const page of settingsPages) {
    page.hidden = page.dataset.settingsPage !== category;
  }
  document.querySelector("[data-settings-heading]").textContent = button.dataset.settingsTitle;
}

function showSettings(external = false) {
  settingsOpenedExternally = external;
  search.blur();
  settingsView.hidden = false;
  launcher.dataset.view = "settings";
  document.querySelector("[data-theme-label]").textContent =
    document.documentElement.dataset.meridianTheme === "light" ? "Hell" : "Dunkel";
  document.querySelector("[data-close-settings]").focus({ preventScroll: true });
}

function selectRadioButton(buttons, selectedValue, dataKey) {
  for (const button of buttons) {
    const selected = button.dataset[dataKey] === selectedValue;
    button.classList.toggle("is-selected", selected);
    button.setAttribute("aria-checked", String(selected));
  }
}

function applyAppearanceState(state) {
  if (!state || !["dark", "light"].includes(state.theme)) {
    return;
  }
  document.documentElement.dataset.meridianTheme = state.theme;
  document.querySelector("[data-theme-label]").textContent =
    state.theme === "light" ? "Hell" : "Dunkel";
  selectRadioButton(appearanceThemeButtons, state.theme, "appearanceTheme");
  wallpaperName.textContent = state.wallpaper_name || "Systemstandard";
  selectRadioButton(wallpaperModeButtons, state.wallpaper_mode, "wallpaperMode");
}

function showApps() {
  settingsOpenedExternally = false;
  launcher.dataset.view = "apps";
  settingsView.hidden = true;
}

function leaveSettings() {
  const closeSurface = settingsOpenedExternally;
  showApps();
  if (closeSurface) {
    requestBridge("launcher.close");
  } else {
    document.querySelector("[data-open-settings]").focus({ preventScroll: true });
  }
}

function returnToApps() {
  showApps();
  document.querySelector("[data-open-settings]").focus({ preventScroll: true });
}

window.meridianLauncher = { applyAppearanceState, showApps, showSettings };

for (const category of categories) {
  category.addEventListener("click", () => activateCategory(category));
  category.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      focusRelative(categories, category, event.key === "ArrowDown" ? 1 : -1);
    }
  });
}

for (const category of settingsCategories) {
  category.addEventListener("click", () => selectSettingsCategory(category));
}

document.querySelector("[data-open-settings]").addEventListener("click", () => {
  showSettings(false);
  requestBridge("settings.appearance.refresh");
});
document.querySelector("[data-close-settings]").addEventListener("click", returnToApps);

for (const button of appearanceThemeButtons) {
  button.addEventListener("click", () => {
    const theme = button.dataset.appearanceTheme;
    applyAppearanceState({
      theme,
      wallpaper_name: wallpaperName.textContent,
      wallpaper_mode: document.querySelector("[data-wallpaper-mode].is-selected")?.dataset.wallpaperMode || "fill",
    });
    requestBridge("settings.appearance.set-theme", { appearance_theme: theme });
  });
}

for (const button of wallpaperModeButtons) {
  button.addEventListener("click", () => {
    selectRadioButton(wallpaperModeButtons, button.dataset.wallpaperMode, "wallpaperMode");
    requestBridge("settings.appearance.set-wallpaper-mode", {
      wallpaper_mode: button.dataset.wallpaperMode,
    });
  });
}

document.querySelector("[data-pick-wallpaper]").addEventListener("click", () => {
  requestBridge("settings.appearance.pick-wallpaper");
});

for (const application of applications) {
  application.addEventListener("focus", () => selectApplication(application));
  application.addEventListener("click", () => {
    selectApplication(application);
    requestBridge("launcher.launch", { desktop_id: application.dataset.appId });
  });
  application.addEventListener("keydown", (event) => {
    const offsets = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -2, ArrowDown: 2 };
    if (Object.hasOwn(offsets, event.key)) {
      event.preventDefault();
      focusRelative(visibleApplications(), application, offsets[event.key]);
    }
  });
}

search.addEventListener("input", applyFilter);
search.addEventListener("keydown", (event) => {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    visibleApplications()[0]?.focus();
  } else if (event.key === "Escape" && search.value) {
    search.value = "";
    applyFilter();
  }
});

document.addEventListener("keydown", (event) => {
  if (launcher.dataset.view === "settings") {
    if (event.key === "Escape") {
      event.preventDefault();
      leaveSettings();
    }
    return;
  }
  const plainText = event.key.length === 1
    && !event.ctrlKey
    && !event.altKey
    && !event.metaKey;
  if (plainText && document.activeElement !== search) {
    event.preventDefault();
    search.focus({ preventScroll: true });
    search.value += event.key;
    applyFilter();
  } else if (event.key.toLocaleLowerCase() === "k" && event.ctrlKey) {
    event.preventDefault();
    search.focus();
  } else if (event.key === "Escape" && !search.value) {
    event.preventDefault();
    search.blur();
    requestBridge("launcher.close");
  }
});

document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    search.blur();
  }
});

applyFilter();
