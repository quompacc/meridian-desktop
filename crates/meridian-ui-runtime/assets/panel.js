(() => {
  "use strict";

  const clock = document.querySelector("[data-clock]");
  const date = document.querySelector("[data-date]");
  const timeFormat = new Intl.DateTimeFormat("de-DE", {
    hour: "2-digit",
    minute: "2-digit",
  });
  const dateFormat = new Intl.DateTimeFormat("de-DE", {
    weekday: "short",
    day: "2-digit",
    month: "2-digit",
  });

  function updateClock() {
    const now = new Date();
    clock.textContent = timeFormat.format(now);
    date.textContent = dateFormat.format(now);
  }

  let requestSequence = 0;
  function post(capability, fields = {}) {
    const handler = window.webkit?.messageHandlers?.meridian;
    if (!handler) {
      return;
    }
    requestSequence += 1;
    handler.postMessage(JSON.stringify({
      version: 1,
      capability,
      request_id: `panel-${requestSequence}`,
      ...fields,
    }));
  }

  document.querySelector(".launcher-button").addEventListener("click", () => {
    post("panel.toggle-launcher");
  });
  document.querySelectorAll(".panel-app").forEach((button) => {
    button.addEventListener("click", () => {
      post("launcher.launch", { desktop_id: button.dataset.appId });
    });
  });

  updateClock();
  window.setInterval(updateClock, 1000);
})();
