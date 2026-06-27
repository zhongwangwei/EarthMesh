#!/usr/bin/env node
"use strict";

const fs = require("fs");

const read = (path) => fs.readFileSync(path, "utf8");
const html = read("gui-tauri/dist/index.html");
const readme = read("gui-tauri/README.md");
const guiLib = read("gui-tauri/src-tauri/src/lib.rs");
const projectLib = read("rust/earthmesh_project/src/lib.rs");
const projectPresets = read("rust/earthmesh_project/src/presets/mod.rs");
const projectCriteria = read("rust/earthmesh_project/src/criteria/mod.rs");
const projectSchema = read("rust/earthmesh_project/src/schema/mod.rs");

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

{
  const rust = `${projectLib}\n${projectPresets}`;
  const ids = [...rust.matchAll(/MeshIntentPreset::[A-Za-z0-9_]+ => "([A-Za-z0-9_]+)"/g)].map(
    (m) => m[1],
  );
  const cards = [...html.matchAll(/intent:"([A-Za-z0-9_]+)"/g)].map((m) => m[1]);
  const missing = cards.filter((id) => !ids.includes(id));
  const hidden = ids.filter((id) => !cards.includes(id));
  check(!missing.length && !hidden.length, "gallery/backend intent mismatch", { missing, hidden });
  log(`mapped ${new Set(cards).size}/${ids.length} backend intents`);
}

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

{
  const dto = read("gui-tauri/src-tauri/src/dto.rs");
  check(
    dto.includes("physical_process: String") &&
      dto.includes("label: String") &&
      dto.includes("help: String") &&
      dto.includes("unit: String") &&
      dto.includes("stem: String") &&
      !dto.includes("display_name: String") &&
      !dto.includes("applicable: Vec<String>") &&
      readme.includes("{physical_process,label,help,unit,stem}"),
    "list_criteria DTO/README drift",
  );
  log("list_criteria DTO shape documented");
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

{
  const body = section(guiLib, /generate_handler!\s*\[([\s\S]*?)\]\)/, "Tauri handler list");
  const cmds = body
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  const docs = [...readme.matchAll(/^\| `([^`]+)` /gm)].map((m) => m[1]);
  const missing = cmds.filter((c) => !docs.includes(c));
  const stale = docs.filter((c) => !cmds.includes(c));
  check(!missing.length && !stale.length, "Tauri command README drift", { missing, stale });
  log(`documented ${cmds.length} Tauri commands`);
}

check(
  readme.includes("layers:[{id,role_kind,role,path,enabled,wants_folder}]"),
  "project_summary README must document layer role_kind/wants_folder",
);
log("project_summary layer shape documented");

{
  const project = `${projectLib}\n${projectCriteria}`;
  check(!project.includes('id: "typhoon"'), "project GUI criterion catalog must not expose unsupported typhoon refinement");
  log("unsupported typhoon criterion stays out of project catalog");
  check(project.includes("id: self.field.stem().to_string()"), "criterion data layers must use engine stems as ids");
  log("criterion data-layer ids use engine stems");
}

{
  const body = section(guiLib, /generate_handler!\s*\[([\s\S]*?)\]\)/, "Tauri handler list");
  const cmds = body
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  const invoked = [...new Set([...html.matchAll(/\b(?:invoke|inv)\("([^"]+)"/g)].map((m) => m[1]))];
  const missing = invoked.filter((c) => !cmds.includes(c));
  check(!missing.length, "frontend invokes unregistered Tauri commands", missing);
  log(`registered ${invoked.length} frontend invoke commands`);
}

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
    "gui-tauri/src-tauri/src/lib.rs": guiLib,
    "rust/earthmesh_project/src/lib.rs": projectLib,
    "rust/earthmesh_project/src/schema/mod.rs": projectSchema,
    "rust/earthmesh_project/src/presets/mod.rs": projectPresets,
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
    "gui-tauri/src-tauri/src/lib.rs": guiLib,
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
    .filter(([, text]) => /\bOLAM\b/.test(text))
    .map(([file]) => file);
  check(!hits.length, "GUI/docs must not expose OLAM as a project output format", hits);
  log("GUI/docs hide deprecated OLAM project output");
}

{
  const rust = `${guiLib}\n${read("gui-tauri/src-tauri/src/project_commands.rs")}\n${read(
    "gui-tauri/src-tauri/src/project_queries.rs",
  )}`;
  check(
    rust.includes("domain_shape") &&
      readme.includes("domain_shape") &&
      html.includes("hiddenDomainShape") &&
      html.includes('kind: "hidden"') &&
      html.includes("preserveDomain: !!hiddenDomainShape") &&
      html.includes("function domainLabel") &&
      html.includes("hiddenDomainShapeText") &&
      html.includes("readyDomain.textContent=domainLabel();") &&
      !html.includes(">${domainLabel()}</b>") &&
      !html.includes("}${hiddenDomain}${") &&
      html.includes('if(hiddenDomainShape){ const el=document.getElementById("estCells");') &&
      rust.includes("preserve_domain: bool"),
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

check(
  html.includes('tr.dataset.path = l.path || "";') &&
    html.includes('tr.dataset.enabled = l.enabled ? "1" : "0";') &&
    html.includes('layerEdits[id] = { path, enabled: tr.dataset.enabled !== "1" };') &&
    !html.includes("const e = layerEdits[id];\n        if (!e || !e.path) return;"),
  "layer toggles must preserve opened project paths",
);
log("layer toggles preserve opened project paths");

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
  const body = section(html, /async function loadQualityAndMesh\(gridfile\) \{([\s\S]*?)\n  \}/, "loadQualityAndMesh body");
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
  const body = section(html, /function renderQualityCard\(q\) \{([\s\S]*?)\n  \}/, "renderQualityCard body");
  check(
    body.includes("metric.textContent = g.metric;") &&
      body.includes('issue.textContent = "\u2022 " + t[0] + ": " + num(t[1]);') &&
      body.includes('b.textContent = "\u25cf " + verdict;') &&
      !body.includes("q.gates.map(chip).join") &&
      !body.includes('b.innerHTML = "\u25cf "'),
    "quality report values must render as text",
  );
  log("quality report values render as text");
}

{
  const core = read("rust/earthmesh_core/src/mkgrd_config/mod.rs");
  const guiSea = Number(html.match(/const DEFAULT_SEA_RATIO_PCT=(\d+(?:\.\d+)?)/)[1]);
  const coreSea = Number(core.match(/mask_sea_ratio:\s*([0-9.]+)/)[1]) * 100;
  const guiAngle = Number(html.match(/inp\("([0-9.]+)."\)/)[1]);
  const rustAngle = Number(projectSchema.match(/DEFAULT_MIN_ANGLE_DEG:\s*f64\s*=\s*([0-9.]+)/)[1]);
  check(guiSea === coreSea && guiAngle === rustAngle, "GUI/backend default drift", {
    guiSea,
    coreSea,
    guiAngle,
    rustAngle,
  });
  log(`GUI defaults match backend: sea ${guiSea}%, min angle ${guiAngle}`);
}

{
  const current = section(html, /function currentResolution\(\) \{([\s\S]*?)\n  \}/, "currentResolution body");
  const nxp = section(html, /function currentNxp\(\) \{([\s\S]*?)\n  \}/, "currentNxp body");
  const res = section(html, /function resInput\(src\)\{([\s\S]*?)\n\}/, "resInput body");
  const reflect = section(html, /async function reflectProject\(res\) \{([\s\S]*?)\n  \}/, "reflectProject body");
  check(
    current.includes("if (resUnitIdx === 1) return { nxp: Math.round(resVal), approxKm: null };") &&
      current.includes("return { nxp: null, approxKm: resVal };") &&
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
  log("frontend quality min angle passes invalid input to Rust validation");
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
  const gui = `${guiLib}\n${read("gui-tauri/src-tauri/src/project_commands.rs")}\n${read(
    "gui-tauri/src-tauri/src/project_edits.rs",
  )}`;
  check(
    html.includes("const refinementEnabled = hasEnabledThresholdLayer(sum);") &&
      html.includes("if (refinementEnabled) {") &&
      html.includes("const shownPasses") &&
      html.includes("no threshold criteria for this template") &&
      html.includes('if (crits.length) anchor.insertAdjacentHTML("afterend", mp);') &&
      html.includes(
        "const refinementPasses = maxPasses == null ? summary.max_passes : (refinementEnabled ? Math.min(9, Math.max(1, maxPasses)) : maxPasses);",
      ) &&
      gui.includes("cfg.refinement.max_passes = if enabled { max_passes } else { 0 };") &&
      projectPresets.includes("max_passes: if d.criteria.is_empty() { 0 } else { 3 }"),
    "disabled refinement max_passes must stay zero/inert",
  );
  log("disabled refinement max_passes zero check passed");
}

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

{
  const project = `${projectLib}\n${projectPresets}`;
  check(
    project.includes("MeshIntentPreset::AtmosphereMpas => (Atmosphere, Hex, Mpas, vec![], vec![])"),
    "atmosphere template must not scaffold unsupported threshold layers",
  );
  log("atmosphere template has no unsupported threshold layers");

  check(
    !html.includes('[["typhoon","\u53f0\u98ce"],["global"') &&
      !html.includes('[["typhoon","\u53f0\u98ce"],["regional"') &&
      !project.includes("Atmosphere \u00b7 Typhoon / Precip"),
    "atmosphere template must not advertise unsupported typhoon refinement",
  );
  log("atmosphere template labels match supported behavior");
}

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
