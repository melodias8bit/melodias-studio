// Read-only dashboard for the melodias history database. It polls one JSON
// summary and redraws; there is no build step and no external dependency.
// Clicking a run opens a detail panel with its media, judge verdict, meta,
// candidates, findings, patches and every artifact on disk.

const REFRESH_MS = 60000;
const TABLE_ROWS = 50;
const SVG_NS = "http://www.w3.org/2000/svg";

// Human names for the deterministic finding codes. The code stays available in
// the tooltip and under the name in the run detail, but the charts read better
// with a plain label.
const FINDING_LABELS = {
  IMG_PALETTE: "Color palette",
  IMG_CONTRAST: "Contrast",
  IMG_SATURATION: "Saturation",
  IMG_GLOW: "Glowing eyes",
  IMG_TEXT: "Text or watermark",
  AUD_HIGHS: "High frequencies",
  AUD_BUILD: "Intro build",
  AUD_DURATION: "Track length",
  AUD_SILENCE: "Silence gaps",
  AUD_LOUDNESS: "Loudness",
  AUD_PEAK: "True peak",
  AUD_CENTROID: "Timbre brightness",
};

function findingLabel(code) {
  return FINDING_LABELS[code] || code;
}

let timer = null;
let busy = false;
let detailId = null;
let lastData = null;
let resizeTimer = null;

// Home view state. The sidebar filters the runs table and the findings chart
// without another request.
const view = { runFilter: "all", findingSource: "all" };

function node(tag, className = "", text = null) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== null) element.textContent = text;
  return element;
}

function svg(tag, attrs = {}, text = null) {
  const element = document.createElementNS(SVG_NS, tag);
  for (const [key, value] of Object.entries(attrs)) {
    element.setAttribute(key, value);
  }
  if (text !== null) element.textContent = text;
  return element;
}

function fmt(value) {
  return value === null || value === undefined || Number.isNaN(value)
    ? "-"
    : Number(value).toFixed(1);
}

function money(value) {
  return value ? `$${Number(value).toFixed(3)}` : "-";
}

function when(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso || "-";
  return date.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function stamp(seconds) {
  if (seconds === null || seconds === undefined) return "-";
  const date = new Date(Number(seconds) * 1000);
  return Number.isNaN(date.getTime()) ? "-" : date.toLocaleString();
}

function shortWhen(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso || "-";
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

function duration(seconds) {
  if (seconds === null || seconds === undefined) return "-";
  if (seconds < 90) return `${Math.round(seconds)}s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 90) return `${minutes}m`;
  return `${(seconds / 3600).toFixed(1)}h`;
}

function bytes(value) {
  if (value === null || value === undefined) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let size = Number(value);
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function showError(message) {
  const banner = document.getElementById("banner");
  banner.textContent = message;
  banner.hidden = false;
}

function statusOf(run) {
  const outcome = run.outcome || (run.accepted ? "accepted" : "below_target");
  if (outcome === "accepted") return { outcome, label: "accepted", kind: "ok" };
  if (outcome === "accepted_by_limit") {
    return { outcome, label: "by limit", kind: "limit" };
  }
  if (outcome === "failed") return { outcome, label: "failed", kind: "low" };
  return { outcome, label: outcome === "below_target" ? "below" : outcome, kind: "low" };
}

function renderLegend() {
  const legend = document.getElementById("legend");
  for (const [className, label] of [
    ["tech", "technical"],
    ["edit", "editorial"],
    ["total", "total"],
  ]) {
    const item = node("span");
    item.append(node("i", `swatch ${className}`), document.createTextNode(label));
    legend.append(item);
  }
}

function bestRun(runs) {
  return runs
    .filter((run) => run.total !== null && run.total !== undefined)
    .reduce((best, run) => (!best || run.total > best.total ? run : best), null);
}

function runMatches(run, filter) {
  const outcome = run.outcome || (run.accepted ? "accepted" : "below_target");
  if (filter === "accepted") return outcome === "accepted";
  if (filter === "by_limit") return outcome === "accepted_by_limit";
  if (filter === "below") return outcome !== "accepted" && outcome !== "accepted_by_limit";
  return true;
}

function runFilterLabel() {
  return (
    {
      all: "all runs",
      accepted: "accepted",
      by_limit: "by limit",
      below: "below target",
    }[view.runFilter] || "all runs"
  );
}

function sourceFilterLabel() {
  return (
    {
      all: "all sources",
      image: "images only",
      audio: "audio only",
    }[view.findingSource] || "all sources"
  );
}

function renderCards(data) {
  const totals = data.totals;
  const best = bestRun(data.runs);
  const latest = data.runs[0] || null;
  const meanCost = totals.runs ? totals.cost_usd / totals.runs : null;
  const meanTime = totals.runs ? totals.seconds / totals.runs : null;
  const cards = [
    ["Runs", String(totals.runs), ""],
    ["Accepted", `${totals.accepted}/${totals.runs}`, ""],
    ["By limit", String(totals.by_limit), ""],
    ["Best total", fmt(best && best.total), best ? best.slug : ""],
    ["Mean total", fmt(totals.total), "target 80"],
    ["Mean technical", fmt(totals.technical), "target 85"],
    ["Mean editorial", fmt(totals.editorial), "target 70"],
    ["Judge spend", money(totals.cost_usd), ""],
    ["Cost per run", money(meanCost), ""],
    ["Mean time", duration(meanTime), ""],
    ["Total time", duration(totals.seconds), ""],
    ["Findings", String(totals.findings), ""],
    ["Lost points", fmt(Math.abs(totals.lost_points)), "points deducted"],
    ["Candidates", String(totals.candidates), ""],
    ["Patches", String(totals.patches), "retry changes"],
    ["Last run", latest ? shortWhen(latest.started_at) : "-", latest ? latest.slug : ""],
  ];
  const container = document.getElementById("cards");
  container.replaceChildren();
  for (const [label, value, hint] of cards) {
    const card = node("div", "card");
    card.append(node("div", "label", label));
    card.append(node("div", "value", value));
    if (hint) {
      const hintNode = node("div", "hint", hint);
      hintNode.title = hint;
      card.append(hintNode);
    }
    container.append(card);
  }
}

function drawSeries(parent, values, className, x, y, runs) {
  let path = "";
  let pen = false;
  values.forEach((value, index) => {
    if (value === null || value === undefined) {
      pen = false;
      return;
    }
    path += `${pen ? "L" : "M"}${x(index).toFixed(1)},${y(value).toFixed(1)} `;
    pen = true;
  });
  if (path) parent.append(svg("path", { d: path, class: `line ${className}` }));
  values.forEach((value, index) => {
    if (value === null || value === undefined) return;
    const dot = svg("circle", {
      cx: x(index),
      cy: y(value),
      r: 3.5,
      class: `dot ${className}`,
    });
    const name =
      className === "tech" ? "technical" : className === "edit" ? "editorial" : "total";
    dot.append(
      svg(
        "title",
        {},
        `#${runs[index].id} · ${name} ${fmt(value)} · ${when(runs[index].started_at)}`,
      ),
    );
    parent.append(dot);
  });
}

function renderScoreChart(data) {
  const chart = document.getElementById("score-chart");
  chart.replaceChildren();
  const runs = [...data.runs].sort((a, b) => a.id - b.id);
  const width = Math.max(640, Math.round(chart.clientWidth || 900));
  const height = 320;
  chart.setAttribute("viewBox", `0 0 ${width} ${height}`);
  const left = 46;
  const right = 18;
  const top = 18;
  const bottom = 38;
  const plotWidth = width - left - right;
  const plotHeight = height - top - bottom;
  const x = (index) =>
    runs.length <= 1 ? left + plotWidth / 2 : left + (index / (runs.length - 1)) * plotWidth;
  const y = (value) => top + plotHeight - (Math.max(0, Math.min(100, value)) / 100) * plotHeight;

  for (const value of [0, 25, 50, 75, 100]) {
    chart.append(
      svg("line", {
        x1: left,
        x2: left + plotWidth,
        y1: y(value),
        y2: y(value),
        class: "grid",
      }),
      svg(
        "text",
        { x: left - 8, y: y(value) + 4, class: "axis", "text-anchor": "end" },
        String(value),
      ),
    );
  }

  chart.append(
    svg("line", {
      x1: left,
      x2: left + plotWidth,
      y1: y(80),
      y2: y(80),
      class: "target",
    }),
    svg("text", { x: left + 6, y: y(80) - 6, class: "target-label" }, "target 80"),
  );

  drawSeries(chart, runs.map((run) => run.technical), "tech", x, y, runs);
  drawSeries(chart, runs.map((run) => run.editorial), "edit", x, y, runs);
  drawSeries(chart, runs.map((run) => run.total), "total", x, y, runs);

  const step = Math.max(1, Math.ceil(runs.length / 8));
  runs.forEach((run, index) => {
    if (index % step !== 0 && index !== runs.length - 1) return;
    chart.append(
      svg(
        "text",
        { x: x(index), y: height - 14, class: "axis", "text-anchor": "middle" },
        `#${run.id}`,
      ),
    );
  });
}

function aggregateFindings(findings) {
  const byCode = new Map();
  for (const finding of findings) {
    if (!finding.delta || finding.delta >= 0) continue;
    const item = byCode.get(finding.code) || {
      code: finding.code,
      lost: 0,
      hits: 0,
    };
    item.lost += finding.delta;
    item.hits += 1;
    byCode.set(finding.code, item);
  }
  return [...byCode.values()].sort((a, b) => a.lost - b.lost);
}

function renderFindingsChart(data) {
  const chart = document.getElementById("findings-chart");
  chart.replaceChildren();
  const findings =
    view.findingSource === "all"
      ? data.findings
      : data.findings.filter((finding) => finding.source === view.findingSource);
  const rows = aggregateFindings(findings).slice(0, 12);
  const totalWidth = Math.max(520, Math.round(chart.clientWidth || 900));
  document.getElementById("findings-hint").textContent =
    `lost points per finding, ${sourceFilterLabel()}`;
  if (!rows.length) {
    chart.setAttribute("viewBox", `0 0 ${totalWidth} 60`);
    chart.append(
      svg(
        "text",
        { x: totalWidth / 2, y: 36, class: "note", "text-anchor": "middle" },
        "No lost points for this selection.",
      ),
    );
    return;
  }
  const height = 20 + rows.length * 30;
  chart.setAttribute("viewBox", `0 0 ${totalWidth} ${height}`);
  const labelWidth = 180;
  const left = labelWidth + 12;
  const valueWidth = 120;
  const width = totalWidth - left - valueWidth;
  const longest = Math.max(...rows.map((row) => Math.abs(row.lost))) || 1;
  const barHeight = 16;

  rows.forEach((row, index) => {
    const top = 20 + index * 30;
    const barWidth = Math.max(3, (Math.abs(row.lost) / longest) * width);
    const label = svg(
      "text",
      { x: labelWidth, y: top + barHeight - 3, class: "row-label", "text-anchor": "end" },
      findingLabel(row.code),
    );
    label.append(svg("title", {}, row.code));
    chart.append(
      label,
      svg("rect", {
        x: left,
        y: top,
        width: barWidth,
        height: barHeight,
        rx: 3,
        class: "bar",
      }),
      svg(
        "text",
        { x: left + barWidth + 8, y: top + barHeight - 3, class: "row-value" },
        `${row.lost.toFixed(1)} · ${row.hits}×`,
      ),
    );
  });
}

function renderOutcomes(data) {
  const chart = document.getElementById("outcomes-chart");
  chart.replaceChildren();
  const totals = data.totals;
  const below = Math.max(0, totals.runs - totals.accepted - totals.by_limit);
  const segments = [
    { label: "Accepted", count: totals.accepted, kind: "ok" },
    { label: "By limit", count: totals.by_limit, kind: "limit" },
    { label: "Below target", count: below, kind: "low" },
  ];
  const width = 420;
  const barHeight = 24;
  let x = 0;
  for (const segment of segments) {
    if (!segment.count || !totals.runs) continue;
    const barWidth = (segment.count / totals.runs) * width;
    chart.append(
      svg("rect", {
        x,
        y: 8,
        width: barWidth,
        height: barHeight,
        rx: 3,
        class: `outcome ${segment.kind}`,
      }),
    );
    x += barWidth;
  }
  if (!totals.runs) {
    chart.append(
      svg(
        "text",
        { x: width / 2, y: 28, class: "note", "text-anchor": "middle" },
        "No runs yet.",
      ),
    );
  }
  const legend = document.getElementById("outcomes-legend");
  legend.replaceChildren();
  for (const segment of segments) {
    const item = node("span");
    item.append(
      node("i", `swatch ${segment.kind}`),
      document.createTextNode(`${segment.label} ${segment.count}`),
    );
    legend.append(item);
  }
}

function scoreCell(value, target) {
  const cell = node("td", "num", fmt(value));
  if (value !== null && value !== undefined) {
    cell.classList.add(value >= target ? "hit" : "miss");
  }
  return cell;
}

function renderRunsTable(data) {
  const table = document.getElementById("runs");
  table.replaceChildren();
  const head = node("thead");
  const headRow = node("tr");
  const columns = [
    ["#", "num"],
    ["When", ""],
    ["Slug", ""],
    ["Tech", "num"],
    ["Edit", "num"],
    ["Total", "num"],
    ["Cost", "num"],
    ["Time", "num"],
    ["Status", ""],
    ["", ""],
  ];
  for (const [label, className] of columns) {
    headRow.append(node("th", className, label));
  }
  head.append(headRow);
  table.append(head);

  const runs = data.runs.filter((run) => runMatches(run, view.runFilter));
  const body = node("tbody");
  if (!runs.length) {
    const row = node("tr");
    const cell = node("td", "note", "No runs match this filter.");
    cell.colSpan = columns.length;
    row.append(cell);
    body.append(row);
  }
  for (const run of runs.slice(0, TABLE_ROWS)) {
    const row = node("tr", "clickable");
    row.tabIndex = 0;
    row.setAttribute("role", "button");
    row.setAttribute("aria-label", `Open run ${run.slug}`);
    row.addEventListener("click", () => openRun(run.id));
    row.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openRun(run.id);
      }
    });
    row.append(node("td", "num", `#${run.id}`));
    row.append(node("td", "mono", when(run.started_at)));
    const slug = node("td", "slug", run.slug);
    slug.title = run.vibe || run.slug;
    row.append(slug);
    row.append(scoreCell(run.technical, 85));
    row.append(scoreCell(run.editorial, 70));
    row.append(scoreCell(run.total, 80));
    row.append(node("td", "num", money(run.cost_usd)));
    row.append(node("td", "num", duration(run.seconds)));
    const status = statusOf(run);
    const statusCell = node("td");
    const badge = node("span", `badge ${status.kind}`, status.label);
    if (run.outcome_reason) badge.title = run.outcome_reason;
    statusCell.append(badge);
    row.append(statusCell);
    row.append(node("td", "open-cell", "Open"));
    body.append(row);
  }
  table.append(body);

  const shown = Math.min(TABLE_ROWS, runs.length);
  document.getElementById("runs-hint").textContent =
    `${shown} of ${data.runs.length} runs · ${runFilterLabel()} · click a run for its details`;
}

function renderEmpty(data) {
  document.getElementById("empty").hidden = false;
  document.getElementById("empty-detail").textContent = data.exists
    ? "The database has no runs yet. Finish a run, or run make backfill."
    : "No history database yet. Finish a run, or run make backfill to score the runs already in output/.";
  for (const id of ["score-panel", "findings-panel", "outcomes-panel", "runs-panel"]) {
    document.getElementById(id).hidden = true;
  }
}

// --- Run detail -----------------------------------------------------------

function openRun(id, updateUrl = true) {
  detailId = id;
  if (updateUrl) {
    history.replaceState(null, "", `?run=${encodeURIComponent(id)}`);
  }
  const dialog = document.getElementById("detail");
  if (!dialog.open) dialog.showModal();
  loadDetail(id);
}

async function loadDetail(id, quiet = false) {
  const body = document.getElementById("detail-body");
  if (!quiet) {
    body.replaceChildren(node("p", "note", "Loading run…"));
  }
  try {
    const response = await fetch(`/api/run?id=${encodeURIComponent(id)}`, {
      cache: "no-store",
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) {
      throw new Error((payload && payload.error) || `HTTP ${response.status}`);
    }
    renderDetail(payload);
  } catch (error) {
    body.replaceChildren();
    const note = node(
      "p",
      "note bad",
      error instanceof Error ? error.message : String(error),
    );
    body.append(note);
  }
}

function section(title, hint = "") {
  const block = node("section", "detail-section");
  const head = node("div", "detail-section-head");
  head.append(node("h3", "", title));
  if (hint) head.append(node("p", "hint", hint));
  block.append(head);
  return block;
}

function facts(rows) {
  const grid = node("div", "facts");
  for (const [label, value] of rows) {
    const fact = node("div", "fact");
    fact.append(node("div", "fact-label", label));
    const shown = value === null || value === undefined || value === "" ? "-" : String(value);
    fact.append(node("div", "fact-value", shown));
    grid.append(fact);
  }
  return grid;
}

function jsonBlock(label, value) {
  if (value === null || value === undefined) return null;
  const details = node("details", "json");
  details.append(node("summary", "", label));
  details.append(node("pre", "code", JSON.stringify(value, null, 2)));
  return details;
}

function artifactMap(detail) {
  return new Map((detail.artifacts || []).map((item) => [item.path, item]));
}

function mediaBlock(label) {
  const block = node("div", "media-block");
  block.append(node("div", "media-label", label));
  return block;
}

function mediaSection(detail) {
  const artifacts = artifactMap(detail);
  const hero = artifacts.get("thumbnail.png");
  const video = artifacts.get("video.mp4") || artifacts.get("video-ffmpeg.mp4");
  const audio = artifacts.get("audio.mp3");
  const master = artifacts.get("audio-master.mp3");
  if (!hero && !video && !audio && !master) return null;

  const block = section("Media");
  if (hero) {
    const item = mediaBlock("Image");
    const link = node("a", "hero-link");
    link.href = hero.url;
    link.target = "_blank";
    link.rel = "noopener";
    const image = node("img", "hero");
    image.src = hero.url;
    image.alt = `Thumbnail for ${detail.slug}`;
    image.loading = "lazy";
    link.append(image);
    item.append(link);
    block.append(item);
  }
  if (video) {
    const item = mediaBlock("Video");
    const player = node("video", "player");
    player.controls = true;
    player.preload = "metadata";
    player.playsInline = true;
    player.src = video.url;
    player.append(node("p", "note", "This browser cannot play the video."));
    item.append(player);
    block.append(item);
  }
  if (audio) {
    const item = mediaBlock("Audio");
    const player = node("audio", "player audio");
    player.controls = true;
    player.preload = "metadata";
    player.src = audio.url;
    item.append(player);
    block.append(item);
  }
  if (master) {
    const item = mediaBlock("Audio master");
    const player = node("audio", "player audio");
    player.controls = true;
    player.preload = "metadata";
    player.src = master.url;
    item.append(player);
    block.append(item);
  }
  return block;
}

function judgeFacts(detail) {
  const verdict = detail.verdict;
  const metaJudge = detail.meta && detail.meta.judge;
  if (!verdict && !metaJudge) return null;
  const omni = verdict && verdict.omni;
  const judge = verdict && verdict.judge;
  const score =
    (omni && omni.best && omni.best.score) ??
    (judge && judge.score) ??
    (metaJudge && metaJudge.score) ??
    null;
  const reason =
    (omni && omni.reason) ?? (judge && judge.reason) ?? (metaJudge && metaJudge.reason) ?? null;
  const selection =
    (verdict && verdict.selection) ??
    (metaJudge ? { image: metaJudge.image, audio: metaJudge.audio } : null);
  const mustFix =
    (omni && omni.must_fix) ??
    (judge && judge.must_fix) ??
    (metaJudge && metaJudge.must_fix) ??
    [];
  return {
    score,
    reason,
    selection,
    mustFix,
    provider: verdict && verdict.provider,
    model: verdict && verdict.model,
    usage: verdict && verdict.usage,
  };
}

function judgeSection(detail) {
  const summary = judgeFacts(detail);
  const raw = jsonBlock("verdict.json", detail.verdict);
  if (!summary && !raw) return null;

  const block = section("Judge verdict");
  if (summary) {
    const rows = [
      ["Score", fmt(summary.score)],
      ["Image", summary.selection && summary.selection.image],
      ["Audio", summary.selection && summary.selection.audio],
      ["Provider", summary.provider],
      ["Model", summary.model],
    ];
    if (summary.usage) {
      const allowance = summary.usage.opencode && summary.usage.opencode.allowance;
      const list = summary.usage.list_cost;
      rows.push(["Allowance", allowance ? money(allowance) : null]);
      rows.push(["List cost", list ? money(list) : null]);
      rows.push(["Tokens", summary.usage.totalTokenCount]);
    }
    block.append(facts(rows));
    if (summary.reason) block.append(node("p", "verdict-reason", summary.reason));
    if (summary.mustFix.length) {
      const list = node("ul", "must-fix");
      for (const item of summary.mustFix) {
        const entry = node("li");
        entry.append(node("span", "chip", item.code || "must fix"));
        if (item.target) entry.append(node("span", "chip soft", item.target));
        if (item.reason) entry.append(node("span", "", item.reason));
        list.append(entry);
      }
      block.append(list);
    }
  }
  if (raw) block.append(raw);
  return block;
}

function candidateArtifact(detail, kind, index) {
  const extension = kind === "image" ? "png" : "wav";
  const name = `${kind}-${String(index).padStart(2, "0")}.${extension}`;
  return (detail.artifacts || []).find((item) => item.name === name) || null;
}

function candidatesSection(detail) {
  const images = (detail.candidates || []).filter((item) => item.kind === "image");
  const audios = (detail.candidates || []).filter((item) => item.kind === "audio");
  if (!images.length && !audios.length) return null;

  const meta = detail.meta || {};
  const prompts = new Map();
  const entries = (meta.image && meta.image.candidates) || [];
  entries.forEach((entry, offset) => {
    const name = String(entry.file || "").split("/").pop();
    prompts.set(name || `image-${String(offset + 1).padStart(2, "0")}.png`, entry);
  });

  const block = section(
    "Candidates",
    "options the review scored, with the score of each side",
  );

  if (images.length) {
    const grid = node("div", "candidate-grid");
    for (const candidate of images) {
      const artifact = candidateArtifact(detail, "image", candidate.idx);
      const promptEntry =
        prompts.get(artifact ? artifact.name : "") ||
        prompts.get(`image-${String(candidate.idx).padStart(2, "0")}.png`);
      const card = node("article", "candidate");
      if (artifact) {
        const link = node("a", "candidate-media");
        link.href = artifact.url;
        link.target = "_blank";
        link.rel = "noopener";
        const image = node("img", "candidate-image");
        image.src = artifact.url;
        image.alt = `Candidate ${candidate.idx} of ${detail.slug}`;
        image.loading = "lazy";
        link.append(image);
        card.append(link);
      }
      const head = node("div", "candidate-head");
      head.append(node("span", "candidate-index", `#${candidate.idx}`));
      head.append(node("span", "candidate-score", fmt(candidate.score)));
      card.append(head);
      if (promptEntry && promptEntry.prompt) {
        const prompt = node("p", "candidate-prompt", promptEntry.prompt);
        prompt.title = promptEntry.prompt;
        card.append(prompt);
      }
      const factsLine = node("p", "candidate-facts", `seed ${candidate.seed ?? "-"}`);
      card.append(factsLine);
      if (candidate.detail) {
        const raw = jsonBlock("score", candidate.detail);
        if (raw) card.append(raw);
      }
      grid.append(card);
    }
    block.append(grid);
  }

  if (audios.length) {
    const list = node("div", "audio-list");
    for (const candidate of audios) {
      const artifact = candidateArtifact(detail, "audio", candidate.idx);
      const item = node("article", "audio-row");
      const head = node("div", "audio-head");
      head.append(node("strong", "", `Audio ${candidate.idx}`));
      head.append(node("span", "candidate-score", fmt(candidate.score)));
      head.append(node("span", "hint", `seed ${candidate.seed ?? "-"}`));
      item.append(head);
      if (artifact) {
        const player = node("audio", "player audio");
        player.controls = true;
        player.preload = "none";
        player.src = artifact.url;
        item.append(player);
      }
      list.append(item);
    }
    block.append(list);
  }
  return block;
}

function findingsSection(detail) {
  if (!detail.findings || !detail.findings.length) return null;
  const block = section("Findings", "the deterministic report for this run");
  const table = node("table", "detail-table");
  const head = node("thead");
  const headRow = node("tr");
  for (const label of ["Finding", "Source", "Severity", "Delta", "Message"]) {
    headRow.append(node("th", label === "Delta" ? "num" : "", label));
  }
  head.append(headRow);
  table.append(head);
  const body = node("tbody");
  for (const finding of detail.findings) {
    const row = node("tr");
    const name = node("td", "finding-name");
    name.append(node("div", "", findingLabel(finding.code)));
    name.append(node("div", "code-hint", finding.code));
    row.append(name);
    row.append(node("td", "", finding.source));
    row.append(node("td", "", finding.severity));
    const delta = node("td", "num", fmt(finding.delta));
    delta.classList.add(finding.delta <= -8 ? "miss" : finding.delta < 0 ? "warn" : "hit");
    row.append(delta);
    const message = node("td", "message", finding.message || "-");
    if (finding.metric) {
      message.title = `${finding.metric}: ${finding.value} (threshold ${finding.threshold})`;
    }
    row.append(message);
    body.append(row);
  }
  table.append(body);
  block.append(table);
  return block;
}

function patchesSection(detail) {
  if (!detail.patches || !detail.patches.length) return null;
  const block = section("Applied patches", "prompt changes made by the retry loop");
  for (const patch of detail.patches) {
    const details = node("details", "patch");
    const summary = node("summary");
    summary.append(node("strong", "", patch.code));
    summary.append(node("span", "hint", ` · ${when(patch.applied_at)}`));
    details.append(summary);
    const grid = node("div", "patch-grid");
    for (const [label, value] of [
      ["before", patch.prompt_before],
      ["after", patch.prompt_after],
    ]) {
      const column = node("div");
      column.append(node("div", "label", label));
      column.append(node("pre", "code", JSON.stringify(value, null, 2)));
      grid.append(column);
    }
    details.append(grid);
    block.append(details);
  }
  return block;
}

function filesSection(detail) {
  if (!detail.artifacts || !detail.artifacts.length) return null;
  const block = section("Files", "everything saved under the run directory");
  const table = node("table", "detail-table");
  const head = node("thead");
  const headRow = node("tr");
  for (const label of ["File", "Kind", "Size", "Modified", ""]) {
    headRow.append(node("th", label === "Size" ? "num" : "", label));
  }
  head.append(headRow);
  table.append(head);
  const body = node("tbody");
  for (const item of detail.artifacts) {
    const row = node("tr");
    const name = node("td", "mono", item.path);
    if (item.kind === "json" || item.kind === "text") name.title = "text file";
    row.append(name);
    row.append(node("td", "", item.kind));
    row.append(node("td", "num", bytes(item.size)));
    row.append(node("td", "mono", stamp(item.modified)));
    const linkCell = node("td");
    const link = node("a", "file-link", "open");
    link.href = item.url;
    link.target = "_blank";
    link.rel = "noopener";
    linkCell.append(link);
    row.append(linkCell);
    body.append(row);
  }
  table.append(body);
  block.append(table);
  return block;
}

function jsonSection(detail) {
  const blocks = [
    ["meta.json", detail.meta],
    ["work/score.json", detail.score],
    ["work/judge/verdict.json", detail.verdict],
    ["config_json (database)", detail.config],
  ]
    .map(([label, value]) => jsonBlock(label, value))
    .filter(Boolean);
  if (!blocks.length) return null;
  const block = section("Raw data", "the stored documents, exactly as written");
  for (const item of blocks) block.append(item);
  return block;
}

function renderDetail(detail) {
  document.getElementById("detail-title").textContent = `#${detail.id} · ${detail.slug}`;
  document.getElementById("detail-sub").textContent = [detail.vibe, when(detail.started_at)]
    .filter(Boolean)
    .join(" · ");

  const body = document.getElementById("detail-body");
  body.replaceChildren();

  const status = statusOf(detail);
  const info = section("Summary");
  info.append(
    facts([
      ["Technical", fmt(detail.technical)],
      ["Editorial", fmt(detail.editorial)],
      ["Total", fmt(detail.total)],
      ["Status", status.label],
      ["Judge spend", money(detail.cost_usd)],
      ["Time", duration(detail.seconds)],
      ["Retry rounds", detail.rounds],
      ["Directory", detail.dir],
    ]),
  );
  if (detail.outcome_reason) {
    const note = node("p", "outcome-note");
    note.append(node("strong", "", "Reason: "), document.createTextNode(detail.outcome_reason));
    info.append(note);
  }
  body.append(info);

  for (const block of [
    mediaSection(detail),
    judgeSection(detail),
    candidatesSection(detail),
    findingsSection(detail),
    patchesSection(detail),
    filesSection(detail),
    jsonSection(detail),
  ]) {
    if (block) body.append(block);
  }

  if (!detail.dir) {
    body.append(
      node(
        "p",
        "note bad",
        "The run directory was not found under the output directory, so the files are not available.",
      ),
    );
  }
}

function renderSidebar(data) {
  const totals = data.totals;
  const below = Math.max(0, totals.runs - totals.accepted - totals.by_limit);
  document.getElementById("count-all").textContent = String(totals.runs);
  document.getElementById("count-accepted").textContent = String(totals.accepted);
  document.getElementById("count-by-limit").textContent = String(totals.by_limit);
  document.getElementById("count-below").textContent = String(below);
  document.getElementById("top-hint").textContent =
    `${totals.runs} runs · ${totals.findings} findings · ${totals.candidates} candidates`;
}

function render(data) {
  document.getElementById("banner").hidden = true;
  lastData = data;
  const updated = new Date(data.generated_at);
  document.getElementById("stamp").textContent = Number.isNaN(updated.getTime())
    ? ""
    : `updated ${updated.toLocaleTimeString()}`;
  renderCards(data);
  renderSidebar(data);
  if (!data.runs.length) {
    renderEmpty(data);
    return;
  }
  for (const id of ["score-panel", "findings-panel", "outcomes-panel", "runs-panel"]) {
    document.getElementById(id).hidden = false;
  }
  document.getElementById("empty").hidden = true;
  renderScoreChart(data);
  renderFindingsChart(data);
  renderOutcomes(data);
  renderRunsTable(data);
}

function activate(button) {
  for (const link of document.querySelectorAll(".nav-link")) {
    const active = link === button;
    link.classList.toggle("is-active", active);
    if (active) {
      link.setAttribute("aria-current", "page");
    } else {
      link.removeAttribute("aria-current");
    }
  }
}

function scrollToPanel(id) {
  document.getElementById(id).scrollIntoView({ behavior: "smooth", block: "start" });
}

for (const button of document.querySelectorAll("[data-nav='overview']")) {
  button.addEventListener("click", () => {
    view.runFilter = "all";
    view.findingSource = "all";
    activate(button);
    if (lastData) {
      renderFindingsChart(lastData);
      renderRunsTable(lastData);
    }
    window.scrollTo({ top: 0, behavior: "smooth" });
  });
}

for (const button of document.querySelectorAll("[data-run-filter]")) {
  button.addEventListener("click", () => {
    view.runFilter = button.dataset.runFilter;
    activate(button);
    if (lastData) renderRunsTable(lastData);
    scrollToPanel("runs-panel");
  });
}

for (const button of document.querySelectorAll("[data-source]")) {
  button.addEventListener("click", () => {
    view.findingSource = button.dataset.source;
    activate(button);
    if (lastData) renderFindingsChart(lastData);
    scrollToPanel("findings-panel");
  });
}

document.getElementById("nav-latest").addEventListener("click", (event) => {
  activate(event.currentTarget);
  if (lastData && lastData.runs.length) openRun(lastData.runs[0].id);
});

document.getElementById("nav-best").addEventListener("click", (event) => {
  activate(event.currentTarget);
  const best = lastData ? bestRun(lastData.runs) : null;
  if (best) openRun(best.id);
});

window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    if (!lastData || !lastData.runs.length) return;
    renderScoreChart(lastData);
    renderFindingsChart(lastData);
    renderOutcomes(lastData);
  }, 150);
});

function schedule() {
  clearTimeout(timer);
  timer = setTimeout(refresh, REFRESH_MS);
}

async function refresh() {
  if (busy || document.hidden) {
    schedule();
    return;
  }
  busy = true;
  try {
    const response = await fetch("/api/summary", { cache: "no-store" });
    const payload = await response.json().catch(() => null);
    if (!response.ok) {
      throw new Error((payload && payload.error) || `HTTP ${response.status}`);
    }
    render(payload);
    document.getElementById("live").classList.remove("down");
  } catch (error) {
    document.getElementById("live").classList.add("down");
    showError(error instanceof Error ? error.message : String(error));
  } finally {
    busy = false;
    schedule();
  }
}

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refresh();
});

document.getElementById("detail-close").addEventListener("click", () => {
  document.getElementById("detail").close();
});

document.getElementById("detail-reload").addEventListener("click", () => {
  if (detailId !== null) loadDetail(detailId);
});

document.getElementById("detail").addEventListener("close", () => {
  detailId = null;
  if (location.search) history.replaceState(null, "", location.pathname);
});

renderLegend();
const initialRun = new URLSearchParams(location.search).get("run");
if (initialRun) openRun(initialRun, false);
refresh();
