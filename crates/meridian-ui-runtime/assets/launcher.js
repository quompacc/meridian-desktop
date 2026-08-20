"use strict";

const search = document.querySelector(".search input");
const heading = document.querySelector("#applications-title");
const headingSummary = document.querySelector(".section-heading p");
const categories = Array.from(document.querySelectorAll(".category"));
const applications = Array.from(document.querySelectorAll(".app"));

let activeCategory = "favorites";
let bridgeRequestId = 0;

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

for (const category of categories) {
  category.addEventListener("click", () => activateCategory(category));
  category.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      focusRelative(categories, category, event.key === "ArrowDown" ? 1 : -1);
    }
  });
}

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
  if (event.key.toLocaleLowerCase() === "k" && event.ctrlKey) {
    event.preventDefault();
    search.focus();
  } else if (event.key === "Escape" && !search.value) {
    event.preventDefault();
    requestBridge("launcher.close");
  }
});

applyFilter();
