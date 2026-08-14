const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

{
  const platform = window.navigator.platform || "";
  if (platform.includes("Mac")) document.body.classList.add("platform-darwin");
  else if (platform.includes("Win")) document.body.classList.add("platform-win32");
  if (globalThis.tauriTrayBridge?.bindWindowControls) {
    globalThis.tauriTrayBridge.bindWindowControls(document);
  }
}

/** @typedef {{
 *  id: string,
 *  displayName: string,
 *  publisher?: string,
 *  displayVersion?: string,
 *  installDate?: string,
 *  installLocation?: string,
 *  uninstallString?: string,
 *  estimatedSizeKb?: number,
 *  protected: boolean,
 *  systemComponent: boolean,
 *  category?: string,
 * }} Program */

/** @type {Program[]} */
let programs = [];
/** @type {Set<string>} */
const selected = new Set();
/** @type {Map<string, {status: string, message?: string}>} */
const statusById = new Map();
let busy = false;
/** @type {"name" | "category" | "publisher" | "version" | "size" | "installDate" | "status"} */
let sortKey = "name";
/** @type {"asc" | "desc"} */
let sortDir = "asc";

const els = {
  rows: document.getElementById("program-rows"),
  search: document.getElementById("search"),
  showSystem: document.getElementById("show-system"),
  refreshBtn: document.getElementById("refresh-btn"),
  selectVisibleBtn: document.getElementById("select-visible-btn"),
  clearBtn: document.getElementById("clear-btn"),
  selectAll: document.getElementById("select-all"),
  uninstallBtn: document.getElementById("uninstall-btn"),
  selectedCount: document.getElementById("selected-count"),
  selectedSize: document.getElementById("selected-size"),
  countLabel: document.getElementById("count-label"),
  elevationBadge: document.getElementById("elevation-badge"),
  emptyState: document.getElementById("empty-state"),
  loadingState: document.getElementById("loading-state"),
  log: document.getElementById("log"),
  clearLogBtn: document.getElementById("clear-log-btn"),
  confirmDialog: document.getElementById("confirm-dialog"),
  confirmList: document.getElementById("confirm-list"),
};

function logLine(message) {
  const stamp = new Date().toLocaleTimeString();
  els.log.textContent += `[${stamp}] ${message}\n`;
  els.log.scrollTop = els.log.scrollHeight;
}

function formatSize(kb) {
  if (kb == null || Number.isNaN(kb)) return "—";
  if (kb < 1024) return `${Math.round(kb)} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `${mb.toFixed(mb < 10 ? 1 : 0)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

function formatInstallDate(value) {
  if (!value) return "—";
  const raw = String(value).trim();
  const match = /^(\d{4})(\d{2})(\d{2})$/.exec(raw);
  if (!match) return raw;
  const date = new Date(
    Number(match[1]),
    Number(match[2]) - 1,
    Number(match[3])
  );
  if (Number.isNaN(date.getTime())) return raw;
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function sumSizeKb(list) {
  let total = 0;
  let known = 0;
  for (const p of list) {
    if (p.estimatedSizeKb != null && !Number.isNaN(p.estimatedSizeKb)) {
      total += p.estimatedSizeKb;
      known += 1;
    }
  }
  return { total, known, missing: list.length - known };
}

function compareText(a, b) {
  return a.localeCompare(b, undefined, { sensitivity: "base", numeric: true });
}

function sortValue(p) {
  switch (sortKey) {
    case "category":
      return (p.category ?? "").toLowerCase();
    case "publisher":
      return (p.publisher ?? "").toLowerCase();
    case "version":
      return (p.displayVersion ?? "").toLowerCase();
    case "size":
      return p.estimatedSizeKb ?? -1;
    case "installDate":
      return p.installDate ?? "";
    case "status":
      return (statusById.get(p.id)?.status ?? "").toLowerCase();
    case "name":
    default:
      return p.displayName.toLowerCase();
  }
}

function visiblePrograms() {
  const q = els.search.value.trim().toLowerCase();
  let list = programs;
  if (q) {
    list = programs.filter((p) => {
      const hay =
        `${p.displayName} ${p.category ?? ""} ${p.publisher ?? ""} ${p.displayVersion ?? ""} ${formatInstallDate(p.installDate)} ${p.installDate ?? ""}`.toLowerCase();
      return hay.includes(q);
    });
  }

  const dir = sortDir === "asc" ? 1 : -1;
  return [...list].sort((a, b) => {
    const av = sortValue(a);
    const bv = sortValue(b);
    if (typeof av === "number" && typeof bv === "number") {
      if (av === bv) return compareText(a.displayName, b.displayName) * dir;
      return (av - bv) * dir;
    }
    const cmp = compareText(String(av), String(bv));
    if (cmp === 0) return compareText(a.displayName, b.displayName) * dir;
    return cmp * dir;
  });
}

function updateSortHeaders() {
  document.querySelectorAll(".sort-btn").forEach((btn) => {
    const key = btn.getAttribute("data-sort");
    const active = key === sortKey;
    btn.setAttribute("aria-pressed", active ? "true" : "false");
    const ind = btn.querySelector(".sort-ind");
    if (ind) ind.textContent = active ? (sortDir === "asc" ? "▲" : "▼") : "";
  });
}

function updateSelectionUi() {
  const selectedProgramsList = programs.filter(
    (p) => selected.has(p.id) && !p.protected
  );
  els.selectedCount.textContent = String(selectedProgramsList.length);
  const selectedSizes = sumSizeKb(selectedProgramsList);
  els.selectedSize.textContent =
    selectedProgramsList.length === 0
      ? "0 KB"
      : selectedSizes.known === 0
        ? "size unknown"
        : formatSize(selectedSizes.total);
  els.uninstallBtn.disabled = busy || selectedProgramsList.length === 0;

  const visible = visiblePrograms().filter((p) => !p.protected);
  const allSelected =
    visible.length > 0 && visible.every((p) => selected.has(p.id));
  els.selectAll.checked = allSelected;
  els.selectAll.indeterminate =
    !allSelected && visible.some((p) => selected.has(p.id));
}

function render() {
  const visible = visiblePrograms();
  const allSizes = sumSizeKb(programs);
  const totalLabel =
    allSizes.known === 0 ? "size unknown" : formatSize(allSizes.total);
  els.countLabel.textContent = `${programs.length} program${
    programs.length === 1 ? "" : "s"
  } · ${totalLabel}`;
  els.loadingState.classList.add("hidden");
  els.emptyState.classList.toggle("hidden", visible.length > 0);

  els.rows.innerHTML = "";
  const frag = document.createDocumentFragment();

  for (const p of visible) {
    const tr = document.createElement("tr");
    if (selected.has(p.id)) tr.classList.add("selected");
    if (p.protected) tr.classList.add("protected");

    const status = statusById.get(p.id);
    const statusText = status?.status ?? "";
    const statusClass = statusText ? ` status-pill ${statusText}` : "status-pill";

    tr.innerHTML = `
      <td class="col-check">
        <input type="checkbox" data-id="${escapeAttr(p.id)}" ${
          selected.has(p.id) ? "checked" : ""
        } ${p.protected || busy ? "disabled" : ""} />
      </td>
      <td class="name-cell">${escapeHtml(p.displayName)}${
        p.protected
          ? ' <span class="muted" title="Protected">(protected)</span>'
          : ""
      }</td>
      <td class="muted">${escapeHtml(p.category ?? "—")}</td>
      <td class="muted">${escapeHtml(p.publisher ?? "—")}</td>
      <td class="muted">${escapeHtml(p.displayVersion ?? "—")}</td>
      <td class="muted">${formatSize(p.estimatedSizeKb)}</td>
      <td class="muted col-date">${escapeHtml(formatInstallDate(p.installDate))}</td>
      <td><span class="${statusClass}">${
        statusText ? escapeHtml(statusText) : "—"
      }</span></td>
    `;
    frag.appendChild(tr);
  }

  els.rows.appendChild(frag);
  updateSortHeaders();
  updateSelectionUi();
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function escapeAttr(value) {
  return escapeHtml(value).replaceAll("`", "&#96;");
}

async function loadPrograms() {
  els.loadingState.classList.remove("hidden");
  els.emptyState.classList.add("hidden");
  try {
    programs = await invoke("list_programs", {
      showSystem: els.showSystem.checked,
    });
    // Drop selections that disappeared
    for (const id of [...selected]) {
      if (!programs.some((p) => p.id === id)) selected.delete(id);
    }
    render();
    logLine(`Loaded ${programs.length} programs.`);
    startBackgroundSizeProbe();
  } catch (err) {
    els.loadingState.classList.add("hidden");
    logLine(`Failed to list programs: ${err}`);
  }
}

function startBackgroundSizeProbe() {
  const missing = programs.filter(
    (p) =>
      (p.estimatedSizeKb == null || Number.isNaN(p.estimatedSizeKb)) &&
      (p.installLocation || p.uninstallString)
  );
  if (!missing.length) return;

  logLine(`Probing folder sizes for ${missing.length} program(s) in the background…`);
  invoke("probe_missing_sizes", {
    items: missing.map((p) => ({
      id: p.id,
      installLocation: p.installLocation ?? null,
      uninstallString: p.uninstallString ?? null,
    })),
  }).catch((err) => {
    logLine(`Size probe failed to start: ${err}`);
  });
}

async function refreshElevation() {
  try {
    const elevated = await invoke("check_elevated");
    if (elevated) {
      els.elevationBadge.textContent = "Administrator";
      els.elevationBadge.className = "auth-badge ok";
    } else {
      els.elevationBadge.textContent = "Not elevated";
      els.elevationBadge.className = "auth-badge pending";
    }
  } catch {
    els.elevationBadge.textContent = "Elevation unknown";
    els.elevationBadge.className = "auth-badge error";
  }
}

function selectedPrograms() {
  return programs.filter((p) => selected.has(p.id) && !p.protected);
}

async function startUninstall() {
  const targets = selectedPrograms();
  if (!targets.length || busy) return;

  els.confirmList.innerHTML = targets
    .map((p) => `<li>${escapeHtml(p.displayName)}</li>`)
    .join("");

  const result = await new Promise((resolve) => {
    const onClose = () => {
      els.confirmDialog.removeEventListener("close", onClose);
      resolve(els.confirmDialog.returnValue);
    };
    els.confirmDialog.addEventListener("close", onClose);
    els.confirmDialog.returnValue = "cancel";
    els.confirmDialog.showModal();
  });

  if (result !== "ok") return;

  busy = true;
  updateSelectionUi();
  render();

  for (const p of targets) {
    statusById.set(p.id, { status: "queued" });
  }
  render();
  logLine(`Starting uninstall of ${targets.length} program(s)…`);

  try {
    await invoke("uninstall_selected", {
      ids: targets.map((p) => p.id),
    });
  } catch (err) {
    logLine(`Uninstall batch error: ${err}`);
  } finally {
    busy = false;
    await loadPrograms();
  }
}

els.rows.addEventListener("change", (e) => {
  const input = e.target;
  if (!(input instanceof HTMLInputElement) || input.type !== "checkbox") return;
  const id = input.dataset.id;
  if (!id) return;
  if (input.checked) selected.add(id);
  else selected.delete(id);
  render();
});

document.querySelector("thead")?.addEventListener("click", (e) => {
  const btn = e.target.closest(".sort-btn");
  if (!(btn instanceof HTMLButtonElement)) return;
  const key = btn.dataset.sort;
  if (!key) return;
  if (sortKey === key) {
    sortDir = sortDir === "asc" ? "desc" : "asc";
  } else {
    sortKey = /** @type {typeof sortKey} */ (key);
    sortDir = key === "size" || key === "installDate" ? "desc" : "asc";
  }
  render();
});

els.search.addEventListener("input", () => render());
els.showSystem.addEventListener("change", () => loadPrograms());
els.refreshBtn.addEventListener("click", () => loadPrograms());
els.clearBtn.addEventListener("click", () => {
  selected.clear();
  render();
});
els.selectVisibleBtn.addEventListener("click", () => {
  for (const p of visiblePrograms()) {
    if (!p.protected) selected.add(p.id);
  }
  render();
});
els.selectAll.addEventListener("change", () => {
  const visible = visiblePrograms().filter((p) => !p.protected);
  if (els.selectAll.checked) {
    for (const p of visible) selected.add(p.id);
  } else {
    for (const p of visible) selected.delete(p.id);
  }
  render();
});
els.uninstallBtn.addEventListener("click", () => startUninstall());
els.clearLogBtn.addEventListener("click", () => {
  els.log.textContent = "";
});

await listen("uninstall-progress", (event) => {
  const payload = event.payload;
  statusById.set(payload.id, {
    status: payload.status,
    message: payload.message,
  });
  const detail = payload.message ? ` — ${payload.message}` : "";
  const code =
    payload.exitCode != null ? ` (exit ${payload.exitCode})` : "";
  logLine(`${payload.displayName}: ${payload.status}${detail}${code}`);
  if (payload.status === "uninstalled") {
    selected.delete(payload.id);
  }
  render();
});

await listen("uninstall-finished", () => {
  logLine("Batch finished.");
});

await listen("program-size", (event) => {
  const payload = event.payload;
  const program = programs.find((p) => p.id === payload.id);
  if (!program) return;
  program.estimatedSizeKb = payload.estimatedSizeKb;
  render();
});

await listen("program-size-finished", () => {
  const missing = sumSizeKb(programs).missing;
  logLine(
    missing > 0
      ? `Folder size probe finished (${missing} still unknown).`
      : "Folder size probe finished."
  );
  render();
});

await listen("tray:action", (event) => {
  if (event.payload === "refresh") {
    loadPrograms();
  }
});

await refreshElevation();
await loadPrograms();
