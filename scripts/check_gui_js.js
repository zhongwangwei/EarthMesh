#!/usr/bin/env node
"use strict";

const fs = require("fs");

const read = (path) => fs.readFileSync(path, "utf8");
const html = read("gui-tauri/dist/index.html");
const readme = read("gui-tauri/README.md");
const capability = read("gui-tauri/src-tauri/capabilities/default.json");

function check(condition, message, details) {
  if (!condition) {
    console.error(message, details || "");
    process.exit(1);
  }
}

function section(text, pattern, name) {
  const match = text.match(pattern);
  check(match, `missing ${name}`);
  return match[1];
}

function log(message) {
  console.log(message);
}

const scripts = [...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/gi)].map(
  (m) => m[1],
);
new Function(scripts.join("\n"));
log(`parsed ${scripts.length} inline scripts`);

check(
  !/<[^>]+\s+on[a-z]+\s*=/i.test(html),
  "frontend must bind events from JavaScript, not inline HTML attributes",
);
log("frontend has no inline HTML event handlers");

check(
  html.includes('href="vendor/leaflet/leaflet.css"') &&
    html.includes('src="vendor/leaflet/leaflet.js"') &&
    !html.includes("unpkg.com") &&
    fs.existsSync("gui-tauri/dist/vendor/leaflet/leaflet.css") &&
    fs.existsSync("gui-tauri/dist/vendor/leaflet/leaflet.js") &&
    fs.existsSync("gui-tauri/dist/vendor/leaflet/LICENSE"),
  "Leaflet runtime and license must be vendored for the desktop CSP",
);
log("Leaflet runtime is vendored locally");

check(
  !html.includes('${watershedPath ?') &&
    !html.includes('${closePath ?') &&
    !html.includes('${specifiedRefine.path ?') &&
    html.includes('watershedText.textContent=watershedPath ?') &&
    html.includes('closeText.textContent=closePath ?') &&
    html.includes('specifiedCloseText.textContent = specifiedRefine.path ?'),
  "project paths must render through textContent, never generated HTML",
);
log("project paths render as text");

check(
  !html.includes("defaultMethodCSpringNestIterations") &&
    html.includes("niterRefine: expertEdit.niterRefine") &&
    html.includes("blank lets the engine choose for the target mesh"),
  "niter_refine must remain unset unless the user explicitly overrides it",
);
log("niter_refine default remains engine-owned");

check(
  html.includes('id="thresholdRefineOn"') &&
    html.includes("thresholdRefine.enabled && hasEnabledThresholdLayer(summary)") &&
    html.includes("thresholdEnabled: !!thresholdRefine.enabled"),
  "threshold refinement must have an independent persisted master switch",
);
log("threshold refinement master switch is wired");

check(
  html.includes('id="refinementStrategySwitches"') &&
    html.includes('id="specifiedRefineOn"') &&
    html.includes('id="thresholdRefineOn"') &&
    html.includes('id="specifiedRefinementPanel"') &&
    html.includes('id="thresholdRefinementPanel"') &&
    html.includes('(specifiedRefine.enabled ? specified : "")') &&
    html.includes('(thresholdRefine.enabled ? threshold : "")'),
  "specified and threshold refinement must be separate panels opened by strategy switches",
);
log("refinement strategies open independent panels");

check(
  html.includes('id="qualityAutoRefineOn"') &&
    html.includes('id="qualityViolationPolicy"') &&
    html.includes('<div class="quality-detail"><span class="quality-tag">') &&
    !html.includes('<div class="quality-detail expert">') &&
    !html.includes("Auto repair attempt") &&
    !html.includes("自动尝试修复"),
  "AutoRefine must remain visible in normal quality controls",
);
log("AutoRefine is visible in normal quality controls");

check(
  html.includes("const autoEligible = !!s;") &&
    html.includes("支持全球、区域、流域；也可从未细化网格开始") &&
    !html.includes('s.domain === "regional" && s.refine_enabled') &&
    !html.includes('qualityEdit.policy = "warn"'),
  "AutoRefine must support every domain and must not be disabled with initial refinement",
);
log("AutoRefine covers all domains and uniform baselines");

check(
  html.includes('.proj-actions{display:flex;flex-direction:row;flex-wrap:nowrap') &&
    html.includes('<div class="proj-actions">'),
  "New/Open/Save must stay in one horizontal project action row",
);
log("project actions stay horizontal");

check(
  /<button[^>]+id="mapEnlargeBtn"[^>]+aria-haspopup="dialog"/.test(html) &&
    html.includes('new WebviewWindow("map"') &&
    html.includes('url: `index.html?view=map&lang=${lang ? "zh" : "en"}`') &&
    html.includes('tauriEvent.emitTo("map", "earthmesh-map-state"') &&
    html.includes('map._resizeObserver=new ResizeObserver(()=>map.invalidateSize({pan:false}))') &&
    capability.includes('"map"') &&
    capability.includes('"core:webview:allow-create-webview-window"'),
  "the enlarged map must open a state-synchronized Tauri window",
);
log("enlarged map opens in a native Tauri window");

check(
  /<input class="proj-name"[^>]*\breadonly\b/.test(html) &&
    html.includes('id="projectNameStep"') &&
    html.includes("if (nameStep && nameTop) nameStep.oninput = () => { nameTop.value = nameStep.value; };"),
  "case name must be a read-only mirror of the editable project name",
);
log("case name follows the project name and is read-only");

check(
  html.includes('${u.ticks.map(t=>`<span>${t}${u.suffix}</span>`).join("")}'),
  "resolution slider ticks must show their unit",
);
log("resolution slider ticks are self-describing");

{
  const body = section(
    html,
    /function applyProjectCapabilities\(capabilities\) \{([\s\S]*?)\n  \}/,
    "applyProjectCapabilities body",
  );
  check(
    html.includes('capabilities: () => invoke("project_capabilities")') &&
      body.includes("capabilities.intent_ids") &&
      body.includes("unsupported gallery intents") &&
      body.includes("capabilities.default_sea_ratio") &&
      body.includes("capabilities.default_min_angle_deg") &&
      body.includes("capabilities.method_c_min_base_nxp") &&
      body.includes("capabilities.method_c_max_refinement_level") &&
      body.includes("capabilities.method_c_spring_nxp1_km") &&
      body.includes("capabilities.km_per_degree_equator") &&
      html.includes("Promise.all([api.capabilities(), api.listCriteria()])") &&
      html.includes("backendReady = loadBackendCapabilities()") &&
      html.includes("if (backendReady) await backendReady;"),
    "runtime project capabilities must gate gallery intents and defaults",
  );
  log("runtime project capabilities own gallery compatibility and limits");
}

check(
  !html.includes("111.32") &&
    html.includes("method_c_spring_nxp1_km:STATIC_BROWSER_METHOD_C_SPRING_NXP1_KM") &&
      html.includes("km_per_degree_equator:STATIC_BROWSER_METHOD_C_SPRING_NXP1_KM/72") &&
    !html.includes("neighbor ratio ≤ 1+g") &&
    !html.includes("邻胞尺寸比 ≤ 1+g"),
  "frontend must use the backend sphere conversion and describe H-field gradation approximately",
);
log("frontend sphere conversion and H-field wording match engine physics");

{
  const def = Number(html.match(/const DEFAULT_TPL=(\d+);/)[1]);
  const cards = [...html.matchAll(/\{intent:"([^"]+)",global:(true|false),nm:\["([^"]+)"/g)].map(
    (m) => ({ intent: m[1], global: m[2] === "true", name: m[3] }),
  );
  const card = cards[def];
  check(card && card.intent === "MeritHydroCoast" && !card.global, "bad default gallery card", {
    def,
    card,
  });
  log(`default gallery card ${def}: ${card.name}`);
}

check(
  !html.includes('meta:["coupled \u00b7 CoLM \u00b7 MERIT-Hydro"') &&
    !html.includes('meta:["land \u00b7 CoLM \u00b7 MERIT-Hydro"'),
  "gallery meta must describe target kind/model/cell, not data source",
);
log("gallery meta uses target defaults");

{
  const stale = [
    "bathy grad",
    'coastline","\u6d77\u5cb8\u7ebf"],ic:"\u2693"',
    'drainage","\u6c47\u6d41"',
    'impervious","\u4e0d\u900f\u6c34"',
    'thermal","\u70ed\u53c2\u6570"',
    "river R2/R3",
  ];
  const hits = stale.filter((s) => html.includes(s));
  check(!hits.length, "gallery tags must match scaffolded data/criteria", hits);
  log("gallery tags match scaffolded data/criteria");
}

check(
  html.includes("function selectTemplate(k)") &&
    html.includes("baseProjectYaml = null;") &&
    html.includes("delete layerEdits[id];") &&
    html.includes("cur = 1;") &&
    html.includes("c.onclick=()=>selectTemplate(+c.dataset.tpl)"),
  "template switch must clear carried project state and advance rail",
);
log("template switch state check passed");

check(!html.includes('head("",STEPS'), "step header helper must not carry an unused argument");
log("step header helper has no dummy argument");

check(
  html.includes('class="pill dom-mode ${domainMode==="watershed"?"on":""}" data-mode="watershed"') &&
    !html.includes("Watershed (unsupported)") &&
    !html.includes("流域（未支持）") &&
    !html.includes("current engine does not accept SHP domains"),
  "watershed SHP must be selectable and described as supported",
);
log("watershed SHP domain entry is enabled");

check(
  !html.includes('value="ProjectConfig"') && !html.includes(">ProjectConfig</b>"),
  "static output placeholders must not show ProjectConfig as a value",
);
log("static output placeholders are neutral");

{
  const body = section(html, /function renderSteps\(\)\{([\s\S]*?)\n\}/, "renderSteps body");
  check(
    !body.includes("innerHTML") &&
      body.includes('el.textContent="";') &&
      body.includes("title.textContent=s.t[lang];") &&
      body.includes("desc.textContent=s.d[lang];"),
    "step rail labels must render as text",
  );
  log("step rail labels render as text");
}

check(
  readme.includes("layers:[{id,role_kind,role,path,enabled,threshold_value,wants_folder}]"),
  "project_summary README must document layer role_kind/wants_folder",
);
log("project_summary layer shape documented");

check(!html.includes('["Cama","CaMa"]'), "GUI must spell CaMa like backend role labels");
log("CaMa label check passed");

{
  const files = {
    "gui-tauri/dist/index.html": html,
    "gui-tauri/README.md": readme,
  };
  const dead = [
    "window.emProject",
    "emProject.",
    "composeYaml, layerEdits",
    "buildFromUi",
    "emProject.scaffold",
    "emProject.composeYaml",
    "scaffold: (",
  ];
  const hits = [];
  for (const [file, text] of Object.entries(files)) {
    for (const needle of dead) if (text.includes(needle)) hits.push(`${file}: ${needle}`);
  }
  check(!hits.length, "dead frontend debug bridge", hits);
  log("dead frontend debug bridge check passed");
}

{
  const files = {
    "gui-tauri/README.md": readme,
    "gui-tauri/dist/index.html": html,
  };
  const banned = [
    "merit_hydro",
    "mkgrd.x <mkgrd.nml>",
    "range,default",
    "replaces static",
    "current template +",
    "Gates & thresholds",
    "\u95e8\u7981\u4e0e\u9608\u503c",
    "augment Run with a real lowered namelist",
    "starting mkgrd.x",
    "\u542f\u52a8 mkgrd.x",
    "top-right header",
    "prototype",
    "mock animation",
    "offlineRun",
    "clearOfflineStats",
    "clearStandaloneStats",
    "standalone fallback",
    "circle/polygon domains",
    'global" (default) for now',
    "step 7 after a run",
    "\u7b2c 7 \u6b65\u663e\u793a",
    "template + resolution + layer edits",
    "name, template, resolution, layer paths",
    "static design reference",
    "rust/earthmesh_gui",
    "Slice 0",
    "Slice 1",
    "Slice 2",
    "Slice 3",
    "Slice 4",
    "Slice 5",
    "SLICE",
    "later slices",
    "data-layer stubs",
    "slope stubs",
    "sidecar/icon work",
    "not from bbox coordinates",
    "Next (iterative)",
  ];
  const hits = [];
  for (const [file, text] of Object.entries(files)) {
    for (const needle of banned) if (text.includes(needle)) hits.push(`${file}: ${needle}`);
  }
  check(!hits.length, "stale GUI/project wording", hits);
  log("stale GUI/project wording check passed");
}

{
  const files = {
    "gui-tauri/README.md": readme,
    "gui-tauri/dist/index.html": html,
  };
  const hits = Object.entries(files)
    .filter(([, text]) => text.includes("AtmosphereTyphoonPrecip"))
    .map(([file]) => file);
  check(!hits.length, "GUI/docs must use AtmosphereMpas intent id", hits);
  log("GUI/docs use AtmosphereMpas intent id");
}

{
  const hits = [
    ["gui-tauri/README.md", readme],
    ["gui-tauri/dist/index.html", html],
  ]
    .filter(([, text]) => /\bMethod-C\b/.test(text))
    .map(([file]) => file);
  check(!hits.length, "GUI/docs must not expose Method-C as a project output format", hits);
  log("GUI/docs hide deprecated Method-C project output");
}

{
  check(
    readme.includes("domain_shape") &&
      html.includes("hiddenDomainShape") &&
      html.includes('kind: "hidden"') &&
      html.includes("preserveDomain: !!hiddenDomainShape") &&
      html.includes("function domainLabel") &&
      html.includes("hiddenDomainShapeText") &&
      html.includes("readyDomain.textContent=domainLabel();") &&
      !html.includes(">${domainLabel()}</b>") &&
      !html.includes("}${hiddenDomain}${") &&
      html.includes('if(hiddenDomainShape){ const el=document.getElementById("estCells");'),
    "hidden regional domain summary drift",
  );
  log("hidden regional domain summary check passed");
}

{
  const unawaitedReflect = /(^|\n)\s*(?!await\s+)reflectProject\(res\);/.test(html);
  check(
    html.includes("renderMissingGridfile") &&
      html.includes("engine did not report gridfile") &&
      html.includes("gridfile: r.gridfile") &&
      html.includes("_lastQuality = null;") &&
      html.includes("runInfo && runInfo.ok && _lastQuality") &&
      html.includes("function setRunControls") &&
      html.includes("if(r){ setRunControls(false);") &&
      html.includes("function clearRunArtifacts") &&
      html.includes("applyMesh(null);") &&
      html.includes("await reflectProject(res);") &&
      html.includes('runOut.textContent=parts.join(" \u00b7 ");') &&
      !html.includes('${runInfo&&runInfo.outdir?(lang?"\u8f93\u51fa\u76ee\u5f55\uff1a":"output: ")+runInfo.outdir') &&
      !unawaitedReflect,
    "run result quality-state drift",
  );
  log("run result quality-state check passed");
}

{
  const body = section(html, /async function killRun\(\)\{([\s\S]*?)\n\}/, "killRun body");
  check(!body.includes("innerHTML +=") && !body.includes("<span"), "kill log must append text, not HTML");
  log("kill log appends text safely");
}

check(
  !html.includes('logbox"); if(lb) lb.innerHTML=""') &&
    !html.includes('logbox"); if (lb) lb.innerHTML = ""'),
  "log clears must use textContent",
);
log("log clears use textContent");

check(!html.includes(".logbox .ok") && !html.includes(".logbox .wn"), "dead log status CSS");
log("dead log status CSS check passed");

{
  const body = section(html, /function enhanceNewProjectStep\(\) \{([\s\S]*?)\n  \}/, "enhanceNewProjectStep body");
  check(
    !body.includes('div.innerHTML = `<span class="path"') &&
      body.includes('label.textContent = "\uD83D\uDCC4 " + (r.name || r.path);') &&
      !html.includes('${outputPath?("\uD83D\uDCC1 "+outputPath)') &&
      body.includes('t0.textContent = "\uD83D\uDCC1 " + outputPath;'),
    "recent projects and output path must render names as text",
  );
  log("recent projects and output path render names as text");
}

check(
  !html.includes("tbody.innerHTML = sum.layers.map") &&
    html.includes('tbody.textContent = "";') &&
    html.includes("id.textContent = l.id;") &&
    html.includes("roleCell.textContent = l.role;") &&
    html.includes("path.textContent = l.path;"),
  "layer rows must render project data as text",
);
log("layer rows render project data as text");

{
  const body = section(
    html,
    /async function enhanceLayerStep\(\) \{([\s\S]*?)\n  \}\n\n  \/\/ ---- domain/,
    "enhanceLayerStep body",
  );
  check(
    body.indexOf("auto.onclick = async") >= 0 &&
      body.indexOf("auto.onclick = async") < body.indexOf("await api.summary") &&
      body.includes("无法读取数据图层") &&
      body.includes("当前模板不需要外部数据图层"),
    "folder matching must bind before project composition and layer loading must expose error/empty states",
  );
  log("data-layer picker binds immediately and reports empty/error states");
}

check(
  html.includes('tr.dataset.path = l.path || "";') &&
    html.includes('tr.dataset.enabled = l.enabled ? "1" : "0";') &&
    html.includes('layerEdits[id] = { path, enabled: tr.dataset.enabled !== "1" };') &&
    !html.includes("const e = layerEdits[id];\n        if (!e || !e.path) return;"),
  "layer toggles must preserve opened project paths",
);
log("layer toggles preserve opened project paths");

check(
  html.includes('layerEdits[l.id] = { path: l.path, enabled: l.enabled };') &&
    !html.includes('sum.layers.forEach((l) => { if (l.path) layerEdits[l.id] = { path: l.path, enabled: true }; });'),
  "opened project layers must preserve disabled state",
);
log("opened project layers preserve disabled state");

{
  const body = section(html, /function renderProjectSummary\(\) \{([\s\S]*?)\n  \}/, "renderProjectSummary body");
  check(
    body.includes("projectSummaryError") &&
      body.includes('sumEl.textContent = "";') &&
      body.includes('err.textContent = (s._err || "").slice(0, 400);') &&
      body.includes("domainEl.textContent = domain;") &&
      body.includes("gateEl.textContent = gate;") &&
      body.includes('layersEl.textContent = on + "/" + total;') &&
      !body.includes('sumEl.innerHTML = ""') &&
      !body.includes(">${domain}</div>") &&
      !body.includes(">${gate}</div>") &&
      !/\$\{\(s\._err \|\| ""\)/.test(body),
    "project summary values must render as text",
  );
  log("project summary values render as text");
}

{
  const body = section(html, /async function loadQualityAndMesh\(gridfile[^)]*\) \{([\s\S]*?)\n  \}/, "loadQualityAndMesh body");
  const note = section(html, /function renderQualityNote\(text\) \{([\s\S]*?)\n  \}/, "renderQualityNote body");
  check(
    html.includes("function renderQualityNote(text)") &&
      body.includes("renderQualityNote") &&
      !body.includes('quality failed: ") + e}</div>') &&
      !note.includes("innerHTML") &&
      note.includes("note.textContent = text;"),
    "quality errors must render as text",
  );
  log("quality errors render as text");
}

{
  const body = section(html, /function paintTargetOutputs\(summary\) \{([\s\S]*?)\n  \}/, "paintTargetOutputs body");
  const qualityBody = section(html, /function readMeshQuality\(gridfile\) \{([\s\S]*?)\n  \}/, "readMeshQuality body");
  check(
    html.includes('id="targetQualityModeOutput"') &&
      html.includes('id="readyQualityModeOutput"') &&
      html.includes("function qualityModeLabel(mode, cell)") &&
      body.includes('const mode = s.quality_mode || (cell === "tri" ? "tri-strict" : "hex-cgrid");') &&
      body.includes('modeIn.textContent = qualityModeLabel(mode, cell);') &&
      body.includes('readyMode.textContent = qualityModeLabel(mode, cell);') &&
      !html.includes('id="targetQualityModeOutput" style="font-size:15px">—</b>') &&
      html.includes('function meshViewKind()') &&
      qualityBody.includes('invoke("mesh_quality", {') &&
      qualityBody.includes('kind: meshViewKind()') &&
      qualityBody.includes('minAngleDeg:') &&
      qualityBody.includes('onViolation:'),
    "quality mode must render user-facing tri/hex labels without an unexplained dash",
  );
  log("quality mode labels are explicit and render from project summary");
}

check(
  html.includes('typeof g.value === "number"') && html.includes(': "N/A";'),
  "quality gate null values must render as N/A",
);
log("quality gate null values render as N/A");

check(
  readme.includes("report `cell_view`") &&
    readme.includes("`tri-strict` for triangle targets") &&
    readme.includes("`hex-cgrid` for hex targets"),
  "GUI README must document quality view selection",
);
log("GUI README documents quality view selection");

{
  const body = section(html, /function renderQualityCard\(q\) \{([\s\S]*?)\n  \}/, "renderQualityCard body");
  check(
    body.includes("metric.textContent = g.metric;") &&
      body.includes("q.cell_view ? field") &&
      body.includes('text("qualityCellView", q.cell_view || "");') &&
      body.includes("const cellSides = (q.cell_sides || []).filter((t) => t[1] > 0);") &&
      body.includes('sideTitle.textContent = z ? "单元边数（观测）" : "Cell sides (observed)";') &&
      body.includes('issue.textContent = "\u2022 " + t[0] + ": " + num(t[1]);') &&
      body.includes('b.textContent = "\u25cf " + verdict;') &&
      !body.includes("q.gates.map(chip).join") &&
      !body.includes('b.innerHTML = "\u25cf "'),
    "quality report values must render as text",
  );
  log("quality report values render as text");
}

{
  const body = section(
    html,
    /function renderAutoRefineDecisions\(decisions\) \{([\s\S]*?)\n  \}/,
    "renderAutoRefineDecisions body",
  );
  check(
    html.includes("auto_refine_decisions") &&
      html.includes('id="autoRefineCard"') &&
      body.includes("reason.textContent =") &&
      body.includes("selected.textContent =") &&
      body.includes("cell.textContent = value == null") &&
      body.includes("reasonText(decision.reason)") &&
      body.includes("preferenceText(regression.preferred)") &&
      body.includes('outcome === "complete"') &&
      body.includes('outcome === "kept"') &&
      body.includes('invoke("open_path", { path })') &&
      !body.includes("innerHTML"),
    "AutoRefine decisions must be returned and rendered as safe text",
  );
  log("AutoRefine decision audit renders as safe text");
}

check(
  html.includes("STATIC_BROWSER_CAPABILITIES") &&
    html.includes("display-only") &&
    html.includes("applyProjectCapabilities(capabilities)") &&
    html.includes("DEFAULT_HFIELD_G = capabilities.default_hfield_g") &&
    html.includes("DEFAULT_OPENMP = capabilities.default_openmp") &&
    html.includes("DEFAULT_NITER = capabilities.default_niter"),
  "Tauri defaults must replace the explicitly bounded plain-browser fallback",
);
log("plain-browser fallback is bounded; Tauri defaults are runtime-owned");

{
  const current = section(html, /function currentResolution\(\) \{([\s\S]*?)\n  \}/, "currentResolution body");
  const nxp = section(html, /function currentNxp\(\) \{([\s\S]*?)\n  \}/, "currentNxp body");
  const res = section(html, /function resInput\(src\)\{([\s\S]*?)\n\}/, "resInput body");
  const reflect = section(html, /async function reflectProject\(res\) \{([\s\S]*?)\n  \}/, "reflectProject body");
  check(
    current.includes("if (resUnitIdx === 1) return { nxp: Math.round(resVal), approxKm: null") &&
      current.includes("return { nxp: null, approxKm: resVal") &&
      current.includes("approxDegree") &&
      !current.includes("if (resVal > 0)") &&
      nxp.includes("lastSummary.effective_nxp != null") &&
      !nxp.includes("r.nxp ||") &&
      !nxp.includes("r.approxKm ||") &&
      res.includes('if(src==="range") v=Math.min(u.max,Math.max(u.min,v));') &&
      !/\n  v=Math\.min\(u\.max,Math\.max\(u\.min,v\)\);\n  resVal/.test(res) &&
      reflect.includes("if (sum.nxp != null)") &&
      reflect.includes("else if (sum.approx_km != null)") &&
      reflect.includes('sum.effective_nxp ?? sum.nxp ?? "?"'),
    "frontend resolution must pass invalid input to Rust validation",
  );
  log("frontend resolution passes invalid input to Rust validation");
}

{
  const body = section(html, /function projectName\(\) \{([\s\S]*?)\n  \}/, "projectName body");
  check(
    /if \(el\) return el\.value\.trim\(\);/.test(body) &&
      !/if \(el && el\.value\.trim\(\)\)/.test(body),
    "frontend project name must pass empty input to Rust validation",
  );
  log("frontend project name passes empty input to Rust validation");
}

{
  const body = section(html, /async function enhanceQualityStep\(\) \{([\s\S]*?)\n  \}/, "enhanceQualityStep body");
  check(body.includes("let minAngle = 0") && !body.includes("let minAngle = 25"), "frontend quality min angle must pass invalid input to Rust validation");
  check(
    body.includes("const s = await refreshSummary();") && !body.includes("let s = lastSummary;"),
    "AutoRefine eligibility must use the current composed project summary",
  );
  log("frontend quality min angle passes invalid input to Rust validation");
  log("AutoRefine eligibility refreshes the project summary");
}

{
  const body = section(html, /const readSeaRatio = \(\) => \{([\s\S]*?)\n    \};/, "readSeaRatio body");
  check(
    body.includes("return isNaN(v) ? null : v / 100;") && !body.includes("Math.max(0, Math.min(100, v))"),
    "frontend sea ratio must pass invalid input to Rust validation",
  );
  log("frontend sea ratio passes invalid input to Rust validation");
}

{
  const body = section(
    html,
    /if \(hfBase\) hfBase\.addEventListener\("input", \(\) => \{([\s\S]*?)\}\);/,
    "h-field base input body",
  );
  check(
    body.includes("Number.isFinite(v) ? v : null") &&
      !body.includes("Number.isFinite(v) && v > 0 ? v : null"),
    "frontend h-field base_m must pass non-positive values to Rust validation",
  );
  log("frontend h-field base_m passes non-positive values to Rust validation");
}

{
  const body = section(html, /async function reflectProject\(res\) \{([\s\S]*?)\n  \}/, "reflectProject body");
  check(body.includes("maxPasses = sum.max_passes;") && !body.includes("if (sum.max_passes)"), "opened project max_passes must not truthy-filter zero");
  log("opened project max_passes preserves zero");
}

{
  const body = section(html, /async function onSave\(\) \{([\s\S]*?)\n  \}/, "onSave body");
  check(
    body.includes("const yaml = await composeYaml();") &&
      body.includes("api.saveProject(yaml)") &&
      body.includes("projectActive = true;") &&
      body.includes("await refreshSummary();") &&
      body.includes("renderProjectSummary();"),
    "save must refresh active project summary",
  );
  log("save refreshes active project summary");
}

{
  check(
    html.includes("const refinementEnabled = (thresholdRefine.enabled && hasEnabledThresholdLayer(sum)) || !!specifiedRefine.enabled;") &&
      !html.includes("const refinementEnabled = regionalRefine ||") &&
      !html.includes("regionalAutoPasses") &&
      html.includes("const shownPasses") &&
      html.includes("no threshold criteria for this template") &&
      html.includes('anchor.insertAdjacentHTML("afterend", mp);') &&
      html.includes(
        "const refinementPasses = refinementEnabled",
      ),
    "disabled refinement max_passes must stay zero/inert",
  );
  log("disabled refinement max_passes zero check passed");
}

check(
  html.includes("METHOD_C_MAX_REFINEMENT_LEVEL = capabilities.method_c_max_refinement_level") &&
    html.includes('const maxRefinePasses = summary.domain === "regional" ? regionalMethodCLevelCap(summary.effective_nxp ?? currentNxp()) : METHOD_C_MAX_REFINEMENT_LEVEL;') &&
    html.includes('const nMax = regionalRefine ? regionalMethodCLevelCap(sum.effective_nxp ?? currentNxp()) : METHOD_C_MAX_REFINEMENT_LEVEL;'),
  "global and regional refinement controls must share the engine level cap",
);
log("Method-C refinement controls share the engine level cap");

{
  const body = section(html, /async function enhanceRefinementStep\(\) \{([\s\S]*?)\n  \}/, "enhanceRefinementStep body");
  check(
    body.includes("label.textContent = c.label;") &&
      body.includes('help.textContent = (c.physical_process || c.help || "") + (c.unit ? " \u00b7 " + c.unit : "");') &&
      !body.includes("const rows = crits.map") &&
      !body.includes("${c.label}") &&
      !body.includes("${c.physical_process || c.help") &&
      !body.includes("/threshold/i.test(l.role"),
    "refinement criteria values must render as text and use role_kind",
  );
  log("refinement criteria values render as text");

  check(
    body.includes('row.dataset.path = l.path || "";') &&
      body.includes('row.dataset.enabled = l.enabled ? "1" : "0";') &&
      body.includes('layerEdits[id] = { path, enabled: row.dataset.enabled !== "1" };') &&
      !body.includes("if (!layerEdits[id]) return;"),
    "refinement toggles must preserve opened project paths",
  );
  log("refinement toggles preserve opened project paths");
}

check(
  !html.includes('[["typhoon","\u53f0\u98ce"],["global"') &&
    !html.includes('[["typhoon","\u53f0\u98ce"],["regional"'),
  "atmosphere template must not advertise unsupported typhoon refinement",
);
log("atmosphere template labels match supported behavior");

{
  const dict = section(html, /const I = \{([\s\S]*?)\n\};/, "i18n dictionary");
  const keys = [...dict.matchAll(/"([^"]+)":\[/g)].map((m) => m[1]);
  const used = [
    ...new Set(
      [...html.matchAll(/data-i18n="([^"]+)"/g)].map((m) => m[1]).concat(
        [...html.matchAll(/L\("([^"]+)"\)/g)].map((m) => m[1]),
      ),
    ),
  ];
  const stale = keys.filter((k) => !used.includes(k));
  const missing = used.filter((k) => !keys.includes(k));
  check(!stale.length && !missing.length, "i18n key drift", { stale, missing });
  log(`checked ${keys.length} i18n keys`);
}
