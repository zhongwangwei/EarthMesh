# Rust build entrypoint for EarthMesh.
# The executable is built from rust/earthmesh_cli. Legacy Fortran sources are
# archived outside the active tree and tracked only by the migration manifest.

CARGO ?= cargo
CLI_MANIFEST = rust/earthmesh_cli/Cargo.toml
CARGO_TARGET_DIR ?= rust/earthmesh_cli/target
BUILD_PROFILE ?= release
EXECUTABLE = mkgrd.x
CLI_FEATURES ?= --features static-netcdf

export CARGO_TARGET_DIR

ifeq ($(BUILD_PROFILE),release)
CARGO_PROFILE_FLAG = --release
CLI_BINARY = $(CARGO_TARGET_DIR)/release/earthmesh_cli
else
CARGO_PROFILE_FLAG =
CLI_BINARY = $(CARGO_TARGET_DIR)/debug/earthmesh_cli
endif

.PHONY: all build clean test test-fast test-gui check-gui-js test-slow test-full fmt fmt-gui clippy clippy-gui clippy-full release-check check-method-c-neighbors

all: build

build:
	$(CARGO) build --manifest-path $(CLI_MANIFEST) $(CARGO_PROFILE_FLAG) $(CLI_FEATURES)
	cp $(CLI_BINARY) $(EXECUTABLE)
	@echo 'EarthMesh Rust executable has been built successfully.'
	@echo 'Executable: $(EXECUTABLE)'

fmt:
	$(CARGO) fmt --manifest-path rust/earthmesh_core/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_geometry/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_mesh/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_quality/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_refine_planner/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_project/Cargo.toml --check
	$(CARGO) fmt --manifest-path rust/earthmesh_cli/Cargo.toml --check

fmt-gui:
	$(CARGO) fmt --manifest-path gui-tauri/src-tauri/Cargo.toml --check

# Lint gate: deny every clippy + rustc warning. Per-crate `[lints.clippy]` in each
# Cargo.toml already allows the intentionally-kept patterns (Fortran-mirroring
# signatures/loops in mesh+cli); anything else fails CI.
# `clippy` = no-netcdf crates (CI fast job); `clippy-full` adds cli (needs NetCDF).
clippy:
	$(CARGO) clippy --manifest-path rust/earthmesh_core/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path rust/earthmesh_project/Cargo.toml --all-targets -- -D warnings

clippy-gui:
	$(CARGO) clippy --manifest-path gui-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings

clippy-full: clippy
	$(CARGO) clippy --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets $(CLI_FEATURES) -- -D warnings

# Fast regression gate: no NetCDF, no GUI — pure Rust crates only. Used by CI's
# `fast` job and as the quick local loop. Builds in seconds (no netcdf-c/HDF5).
test-fast:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_project/Cargo.toml --all-targets

check-gui-js:
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const scripts=[...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/gi)].map(m=>m[1]); new Function(scripts.join("\n")); console.log("parsed "+scripts.length+" inline scripts");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const rust=fs.readFileSync("rust/earthmesh_project/src/lib.rs","utf8")+"\n"+fs.readFileSync("rust/earthmesh_project/src/presets/mod.rs","utf8"); const ids=[...rust.matchAll(/MeshIntentPreset::[A-Za-z0-9_]+ => "([A-Za-z0-9_]+)"/g)].map(m=>m[1]); const cards=[...html.matchAll(/intent:"([A-Za-z0-9_]+)"/g)].map(m=>m[1]); const missing=cards.filter(id=>!ids.includes(id)); const hidden=ids.filter(id=>!cards.includes(id)); if(missing.length||hidden.length){ console.error("gallery/backend intent mismatch", {missing, hidden}); process.exit(1); } console.log("mapped "+new Set(cards).size+"/"+ids.length+" backend intents");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const def=Number(html.match(/const DEFAULT_TPL=(\d+);/)[1]); const cards=[...html.matchAll(/\{intent:"([^"]+)",global:(true|false),nm:\["([^"]+)"/g)].map(m=>({intent:m[1],global:m[2]==="true",name:m[3]})); const card=cards[def]; if(!card || card.intent!=="MeritHydroCoast" || card.global){ console.error("bad default gallery card", {def, card}); process.exit(1); } console.log("default gallery card "+def+": "+card.name);'
	node -e 'const fs=require("fs"); const dto=fs.readFileSync("gui-tauri/src-tauri/src/dto.rs","utf8"); const readme=fs.readFileSync("gui-tauri/README.md","utf8"); if(!dto.includes("physical_process: String") || !dto.includes("label: String") || !dto.includes("help: String") || !dto.includes("unit: String") || !dto.includes("stem: String") || dto.includes("display_name: String") || dto.includes("applicable: Vec<String>") || !readme.includes("{physical_process,label,help,unit,stem}")){ console.error("list_criteria DTO/README drift"); process.exit(1); } console.log("list_criteria DTO shape documented");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes("meta:[\"coupled · CoLM · MERIT-Hydro\"") || html.includes("meta:[\"land · CoLM · MERIT-Hydro\"")){ console.error("gallery meta must describe target kind/model/cell, not data source"); process.exit(1); } console.log("gallery meta uses target defaults");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const stale=["bathy grad","coastline\",\"海岸线\"],ic:\"⚓","drainage\",\"汇流\"","impervious\",\"不透水\"","thermal\",\"热参数\"","river R2/R3"]; const hits=stale.filter(s=>html.includes(s)); if(hits.length){ console.error("gallery tags must match scaffolded data/criteria", hits); process.exit(1); } console.log("gallery tags match scaffolded data/criteria");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(!html.includes("function selectTemplate(k)") || !html.includes("baseProjectYaml = null;") || !html.includes("delete layerEdits[id];") || !html.includes("cur = 1;") || !html.includes("c.onclick=()=>selectTemplate(+c.dataset.tpl)")){ console.error("template switch must clear carried project state and advance rail"); process.exit(1); } console.log("template switch state check passed");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes("head(\\\"\\\",STEPS")){ console.error("step header helper must not carry an unused argument"); process.exit(1); } console.log("step header helper has no dummy argument");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes("value=\"ProjectConfig\"") || html.includes(">ProjectConfig</b>")){ console.error("static output placeholders must not show ProjectConfig as a value"); process.exit(1); } console.log("static output placeholders are neutral");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/function renderSteps\(\)\{([\s\S]*?)\n\}/)[1]; if(body.includes("innerHTML") || !body.includes("el.textContent=\"\";") || !body.includes("title.textContent=s.t[lang];") || !body.includes("desc.textContent=s.d[lang];")){ console.error("step rail labels must render as text"); process.exit(1); } console.log("step rail labels render as text");'
	node -e 'const fs=require("fs"); const rust=fs.readFileSync("gui-tauri/src-tauri/src/lib.rs","utf8"); const readme=fs.readFileSync("gui-tauri/README.md","utf8"); const body=rust.match(/generate_handler!\s*\[([\s\S]*?)\]\)/)[1]; const cmds=body.split(",").map(s=>s.trim()).filter(Boolean); const docs=[...readme.matchAll(/^\| `([^`]+)` /gm)].map(m=>m[1]); const missing=cmds.filter(c=>!docs.includes(c)); const stale=docs.filter(c=>!cmds.includes(c)); if(missing.length||stale.length){ console.error("Tauri command README drift", {missing, stale}); process.exit(1); } console.log("documented "+cmds.length+" Tauri commands");'
	node -e 'const fs=require("fs"); const readme=fs.readFileSync("gui-tauri/README.md","utf8"); if(!readme.includes("layers:[{id,role_kind,role,path,enabled,wants_folder}]")){ console.error("project_summary README must document layer role_kind/wants_folder"); process.exit(1); } console.log("project_summary layer shape documented");'
	node -e 'const fs=require("fs"); const project=fs.readFileSync("rust/earthmesh_project/src/lib.rs","utf8")+"\n"+fs.readFileSync("rust/earthmesh_project/src/criteria/mod.rs","utf8"); if(project.includes("id: \"typhoon\"")){ console.error("project GUI criterion catalog must not expose unsupported typhoon refinement"); process.exit(1); } console.log("unsupported typhoon criterion stays out of project catalog");'
	node -e 'const fs=require("fs"); const project=fs.readFileSync("rust/earthmesh_project/src/lib.rs","utf8")+"\n"+fs.readFileSync("rust/earthmesh_project/src/criteria/mod.rs","utf8"); if(!project.includes("id: self.field.stem().to_string()")){ console.error("criterion data layers must use engine stems as ids"); process.exit(1); } console.log("criterion data-layer ids use engine stems");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const rust=fs.readFileSync("gui-tauri/src-tauri/src/lib.rs","utf8"); const body=rust.match(/generate_handler!\s*\[([\s\S]*?)\]\)/)[1]; const cmds=body.split(",").map(s=>s.trim()).filter(Boolean); const invoked=[...new Set([...html.matchAll(/\b(?:invoke|inv)\("([^"]+)"/g)].map(m=>m[1]))]; const missing=invoked.filter(c=>!cmds.includes(c)); if(missing.length){ console.error("frontend invokes unregistered Tauri commands", missing); process.exit(1); } console.log("registered "+invoked.length+" frontend invoke commands");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes("[\"Cama\",\"CaMa\"]")){ console.error("GUI must spell CaMa like backend role labels"); process.exit(1); } console.log("CaMa label check passed");'
	node -e 'const fs=require("fs"); const files=["gui-tauri/dist/index.html","gui-tauri/README.md"]; const dead=["buildFromUi","emProject.scaffold","emProject.composeYaml","scaffold: ("]; const hits=[]; for(const f of files){ const text=fs.readFileSync(f,"utf8"); for(const s of dead){ if(text.includes(s)) hits.push(f+": "+s); } } if(hits.length){ console.error("dead frontend debug bridge", hits); process.exit(1); } console.log("dead frontend debug bridge check passed");'
	node -e 'const fs=require("fs"); const files=["gui-tauri/README.md","gui-tauri/dist/index.html","gui-tauri/src-tauri/src/lib.rs","rust/earthmesh_project/src/lib.rs","rust/earthmesh_project/src/schema/mod.rs","rust/earthmesh_project/src/presets/mod.rs"]; const banned=["merit_hydro","mkgrd.x <mkgrd.nml>","range,default","replaces static","current template +","Gates & thresholds","门禁与阈值","augment Run with a real lowered namelist","starting mkgrd.x","启动 mkgrd.x","top-right header","prototype","mock animation","offlineRun","clearOfflineStats","clearStandaloneStats","standalone fallback","circle/polygon domains","global\" (default) for now","step 7 after a run","第 7 步显示","template + resolution + layer edits","name, template, resolution, layer paths","static design reference","rust/earthmesh_gui","Slice 0","Slice 1","Slice 2","Slice 3","Slice 4","Slice 5","SLICE","later slices","data-layer stubs","slope stubs","sidecar/icon work","not from bbox coordinates","Next (iterative)"]; const hits=[]; for(const f of files){ const text=fs.readFileSync(f,"utf8"); for(const b of banned){ if(text.includes(b)) hits.push(f+": "+b); } } if(hits.length){ console.error("stale GUI/project wording", hits); process.exit(1); } console.log("stale GUI/project wording check passed");'
	node -e 'const fs=require("fs"); const files=["gui-tauri/README.md","gui-tauri/dist/index.html","gui-tauri/src-tauri/src/lib.rs"]; const hits=files.filter(f=>fs.readFileSync(f,"utf8").includes("AtmosphereTyphoonPrecip")); if(hits.length){ console.error("GUI/docs must use AtmosphereMpas intent id", hits); process.exit(1); } console.log("GUI/docs use AtmosphereMpas intent id");'
	node -e 'const fs=require("fs"); const files=["gui-tauri/README.md","gui-tauri/dist/index.html"]; const hits=files.filter(f=>/\\bOLAM\\b/.test(fs.readFileSync(f,"utf8"))); if(hits.length){ console.error("GUI/docs must not expose OLAM as a project output format", hits); process.exit(1); } console.log("GUI/docs hide deprecated OLAM project output");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const rust=fs.readFileSync("gui-tauri/src-tauri/src/lib.rs","utf8")+"\n"+fs.readFileSync("gui-tauri/src-tauri/src/project_commands.rs","utf8")+"\n"+fs.readFileSync("gui-tauri/src-tauri/src/project_queries.rs","utf8"); const readme=fs.readFileSync("gui-tauri/README.md","utf8"); if(!rust.includes("domain_shape") || !readme.includes("domain_shape") || !html.includes("hiddenDomainShape") || !html.includes("kind: \"hidden\"") || !html.includes("preserveDomain: !!hiddenDomainShape") || !html.includes("function domainLabel") || !html.includes("hiddenDomainShapeText") || !html.includes("readyDomain.textContent=domainLabel();") || html.includes(">$${domainLabel()}</b>") || html.includes("}$${hiddenDomain}$${") || !html.includes("if(hiddenDomainShape){ const el=document.getElementById(\"estCells\");") || !rust.includes("preserve_domain: bool")){ console.error("hidden regional domain summary drift"); process.exit(1); } console.log("hidden regional domain summary check passed");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const unawaitedReflect=/(^|\n)\s*(?!await\s+)reflectProject\(res\);/.test(html); if(!html.includes("renderMissingGridfile") || !html.includes("engine did not report gridfile") || !html.includes("gridfile: r.gridfile") || !html.includes("_lastQuality = null;") || !html.includes("runInfo && runInfo.ok && _lastQuality") || !html.includes("function setRunControls") || !html.includes("if(r){ setRunControls(false);") || !html.includes("function clearRunArtifacts") || !html.includes("applyMesh(null);") || !html.includes("await reflectProject(res);") || !html.includes("runOut.textContent=parts.join(\" · \");") || html.includes("$${runInfo&&runInfo.outdir?(lang?\"输出目录：\":\"output: \")+runInfo.outdir") || unawaitedReflect){ console.error("run result quality-state drift"); process.exit(1); } console.log("run result quality-state check passed");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function killRun\(\)\{([\s\S]*?)\n\}/)[1]; if(body.includes("innerHTML +=") || body.includes("<span")){ console.error("kill log must append text, not HTML"); process.exit(1); } console.log("kill log appends text safely");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes("logbox\"); if(lb) lb.innerHTML=\"\"") || html.includes("logbox\"); if (lb) lb.innerHTML = \"\"")){ console.error("log clears must use textContent"); process.exit(1); } console.log("log clears use textContent");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes(".logbox .ok") || html.includes(".logbox .wn")){ console.error("dead log status CSS"); process.exit(1); } console.log("dead log status CSS check passed");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/function enhanceNewProjectStep\(\) \{([\s\S]*?)\n  \}/)[1]; if(body.includes("div.innerHTML = `<span class=\"path\"") || !body.includes("label.textContent = \"📄 \" + (r.name || r.path);") || html.includes("$${outputPath?(\"📁 \"+outputPath)") || !body.includes("t0.textContent = \"📁 \" + outputPath;")){ console.error("recent projects and output path must render names as text"); process.exit(1); } console.log("recent projects and output path render names as text");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(html.includes("tbody.innerHTML = sum.layers.map") || !html.includes("tbody.textContent = \"\";") || !html.includes("id.textContent = l.id;") || !html.includes("roleCell.textContent = l.role;") || !html.includes("path.textContent = l.path;")){ console.error("layer rows must render project data as text"); process.exit(1); } console.log("layer rows render project data as text");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); if(!html.includes("tr.dataset.path = l.path || \"\";") || !html.includes("tr.dataset.enabled = l.enabled ? \"1\" : \"0\";") || !html.includes("layerEdits[id] = { path, enabled: tr.dataset.enabled !== \"1\" };") || html.includes("const e = layerEdits[id];\\n        if (!e || !e.path) return;")){ console.error("layer toggles must preserve opened project paths"); process.exit(1); } console.log("layer toggles preserve opened project paths");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/function renderProjectSummary\(\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("projectSummaryError") || !body.includes("sumEl.textContent = \"\";") || !body.includes("err.textContent = (s._err || \"\").slice(0, 400);") || !body.includes("domainEl.textContent = domain;") || !body.includes("gateEl.textContent = gate;") || !body.includes("layersEl.textContent = on + \"/\" + total;") || body.includes("sumEl.innerHTML = \"\"") || body.includes(">$${domain}</div>") || body.includes(">$${gate}</div>") || /\\$\\{\\(s\\._err \\|\\| \"\"\\)/.test(body)){ console.error("project summary values must render as text"); process.exit(1); } console.log("project summary values render as text");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function loadQualityAndMesh\(gridfile\) \{([\s\S]*?)\n  \}/)[1]; const note=html.match(/function renderQualityNote\(text\) \{([\s\S]*?)\n  \}/)[1]; if(!html.includes("function renderQualityNote(text)") || !body.includes("renderQualityNote") || body.includes("quality failed: \") + e}</div>") || note.includes("innerHTML") || !note.includes("note.textContent = text;")){ console.error("quality errors must render as text"); process.exit(1); } console.log("quality errors render as text");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/function renderQualityCard\(q\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("metric.textContent = g.metric;") || !body.includes("issue.textContent = \"• \" + t[0] + \": \" + num(t[1]);") || !body.includes("b.textContent = \"● \" + verdict;") || body.includes("q.gates.map(chip).join") || body.includes("b.innerHTML = \"● \"")){ console.error("quality report values must render as text"); process.exit(1); } console.log("quality report values render as text");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const project=fs.readFileSync("rust/earthmesh_project/src/schema/mod.rs","utf8"); const core=fs.readFileSync("rust/earthmesh_core/src/mkgrd_config/mod.rs","utf8"); const guiSea=Number(html.match(/const DEFAULT_SEA_RATIO_PCT=(\d+(?:\.\d+)?)/)[1]); const coreSea=Number(core.match(/mask_sea_ratio:\s*([0-9.]+)/)[1])*100; const guiAngle=Number(html.match(/inp\("([0-9.]+)°"\)/)[1]); const rustAngle=Number(project.match(/DEFAULT_MIN_ANGLE_DEG:\s*f64\s*=\s*([0-9.]+)/)[1]); if(guiSea!==coreSea || guiAngle!==rustAngle){ console.error("GUI/backend default drift", {guiSea, coreSea, guiAngle, rustAngle}); process.exit(1); } console.log("GUI defaults match backend: sea "+guiSea+"%, min angle "+guiAngle+"°");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const current=html.match(/function currentResolution\(\) \{([\s\S]*?)\n  \}/)[1]; const nxp=html.match(/function currentNxp\(\) \{([\s\S]*?)\n  \}/)[1]; const res=html.match(/function resInput\(src\)\{([\s\S]*?)\n\}/)[1]; const reflect=html.match(/async function reflectProject\(res\) \{([\s\S]*?)\n  \}/)[1]; if(!current.includes("if (resUnitIdx === 1) return { nxp: Math.round(resVal), approxKm: null };") || !current.includes("return { nxp: null, approxKm: resVal };") || current.includes("if (resVal > 0)") || !nxp.includes("lastSummary.effective_nxp != null") || nxp.includes("r.nxp ||") || nxp.includes("r.approxKm ||") || !res.includes("if(src===\"range\") v=Math.min(u.max,Math.max(u.min,v));") || /\n  v=Math\.min\(u\.max,Math\.max\(u\.min,v\)\);\n  resVal/.test(res) || !reflect.includes("if (sum.nxp != null)") || !reflect.includes("else if (sum.approx_km != null)") || !reflect.includes("sum.effective_nxp ?? sum.nxp ?? \"?\"")){ console.error("frontend resolution must pass invalid input to Rust validation"); process.exit(1); } console.log("frontend resolution passes invalid input to Rust validation");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/function projectName\(\) \{([\s\S]*?)\n  \}/)[1]; if(!/if \(el\) return el\.value\.trim\(\);/.test(body) || /if \(el && el\.value\.trim\(\)\)/.test(body)){ console.error("frontend project name must pass empty input to Rust validation"); process.exit(1); } console.log("frontend project name passes empty input to Rust validation");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function enhanceQualityStep\(\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("let minAngle = 0") || body.includes("let minAngle = 25")){ console.error("frontend quality min angle must pass invalid input to Rust validation"); process.exit(1); } console.log("frontend quality min angle passes invalid input to Rust validation");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/const readSeaRatio = \(\) => \{([\s\S]*?)\n    \};/)[1]; if(!body.includes("return isNaN(v) ? null : v / 100;") || body.includes("Math.max(0, Math.min(100, v))")){ console.error("frontend sea ratio must pass invalid input to Rust validation"); process.exit(1); } console.log("frontend sea ratio passes invalid input to Rust validation");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function reflectProject\(res\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("maxPasses = sum.max_passes;") || body.includes("if (sum.max_passes)")){ console.error("opened project max_passes must not truthy-filter zero"); process.exit(1); } console.log("opened project max_passes preserves zero");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function onSave\(\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("const yaml = await composeYaml();") || !body.includes("api.saveProject(yaml)") || !body.includes("projectActive = true;") || !body.includes("await refreshSummary();") || !body.includes("renderProjectSummary();")){ console.error("save must refresh active project summary"); process.exit(1); } console.log("save refreshes active project summary");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const gui=fs.readFileSync("gui-tauri/src-tauri/src/lib.rs","utf8")+"\n"+fs.readFileSync("gui-tauri/src-tauri/src/project_commands.rs","utf8")+"\n"+fs.readFileSync("gui-tauri/src-tauri/src/project_edits.rs","utf8"); const project=fs.readFileSync("rust/earthmesh_project/src/presets/mod.rs","utf8"); if(!html.includes("const refinementEnabled = hasEnabledThresholdLayer(sum);") || !html.includes("if (refinementEnabled) {") || !html.includes("const shownPasses") || !html.includes("no threshold criteria for this template") || !html.includes("if (crits.length) anchor.insertAdjacentHTML(\"afterend\", mp);") || !html.includes("const refinementPasses = maxPasses == null ? summary.max_passes : (refinementEnabled ? Math.min(9, Math.max(1, maxPasses)) : maxPasses);") || !gui.includes("cfg.refinement.max_passes = if enabled { max_passes } else { 0 };") || !project.includes("max_passes: if d.criteria.is_empty() { 0 } else { 3 }")){ console.error("disabled refinement max_passes must stay zero/inert"); process.exit(1); } console.log("disabled refinement max_passes zero check passed");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function enhanceRefinementStep\(\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("label.textContent = c.label;") || !body.includes("help.textContent = (c.physical_process || c.help || \"\") + (c.unit ? \" · \" + c.unit : \"\");") || body.includes("const rows = crits.map") || body.includes("$${c.label}") || body.includes("$${c.physical_process || c.help") || body.includes("/threshold/i.test(l.role")){ console.error("refinement criteria values must render as text and use role_kind"); process.exit(1); } console.log("refinement criteria values render as text");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const body=html.match(/async function enhanceRefinementStep\(\) \{([\s\S]*?)\n  \}/)[1]; if(!body.includes("row.dataset.path = l.path || \"\";") || !body.includes("row.dataset.enabled = l.enabled ? \"1\" : \"0\";") || !body.includes("layerEdits[id] = { path, enabled: row.dataset.enabled !== \"1\" };") || body.includes("if (!layerEdits[id]) return;")){ console.error("refinement toggles must preserve opened project paths"); process.exit(1); } console.log("refinement toggles preserve opened project paths");'
	node -e 'const fs=require("fs"); const project=fs.readFileSync("rust/earthmesh_project/src/lib.rs","utf8")+"\n"+fs.readFileSync("rust/earthmesh_project/src/presets/mod.rs","utf8"); if(!project.includes("MeshIntentPreset::AtmosphereMpas => (Atmosphere, Hex, Mpas, vec![], vec![])")){ console.error("atmosphere template must not scaffold unsupported threshold layers"); process.exit(1); } console.log("atmosphere template has no unsupported threshold layers");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const project=fs.readFileSync("rust/earthmesh_project/src/lib.rs","utf8")+"\n"+fs.readFileSync("rust/earthmesh_project/src/presets/mod.rs","utf8"); if(html.includes("[[\"typhoon\",\"台风\"],[\"global\"") || html.includes("[[\"typhoon\",\"台风\"],[\"regional\"") || project.includes("Atmosphere · Typhoon / Precip")){ console.error("atmosphere template must not advertise unsupported typhoon refinement"); process.exit(1); } console.log("atmosphere template labels match supported behavior");'
	node -e 'const fs=require("fs"); const html=fs.readFileSync("gui-tauri/dist/index.html","utf8"); const dict=html.match(/const I = \{([\s\S]*?)\n\};/)[1]; const keys=[...dict.matchAll(/"([^"]+)":\[/g)].map(m=>m[1]); const used=[...new Set([...html.matchAll(/data-i18n="([^"]+)"/g)].map(m=>m[1]).concat([...html.matchAll(/L\("([^"]+)"\)/g)].map(m=>m[1])))]; const stale=keys.filter(k=>!used.includes(k)); const missing=used.filter(k=>!keys.includes(k)); if(stale.length||missing.length){ console.error("i18n key drift", {stale, missing}); process.exit(1); } console.log("checked "+keys.length+" i18n keys");'

test-gui: check-gui-js
	$(CARGO) test --manifest-path gui-tauri/src-tauri/Cargo.toml --all-targets

# Full crate tests (includes cli with static-netcdf — slow first build).
test:
	$(CARGO) test --manifest-path rust/earthmesh_core/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_geometry/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_mesh/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_quality/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_refine_planner/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_project/Cargo.toml --all-targets
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets $(CLI_FEATURES)

test-slow:
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_mask_restart $(CLI_FEATURES) -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test colm_coupling_csv_from_mesh $(CLI_FEATURES) mesh_plus_landtype_classifies_cells_and_writes_colm_netcdf -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test colm_coupling_csv_from_mesh $(CLI_FEATURES) mesh_plus_landtype_coupling_quality_report -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test hydro_workflow $(CLI_FEATURES) full_chain_with_mesh_landtype_coupling_quality -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test refine_end_to_end_topology $(CLI_FEATURES) specified_bbox_refine_produces_consistent_closed_mpas -- --ignored
	$(CARGO) test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_gridinit $(CLI_FEATURES) run_mkgrd_gridinit_global_matches_fortran_nxp64_gridfile_fixture -- --ignored

test-full: check-method-c-neighbors test test-gui test-slow

# Release fast gate: format + no-netcdf crates. Run before tagging a release; the
# full gate adds `make test-full` (GUI + CLI/static-netcdf + ignored slow tests) on top.
release-check: fmt test-fast
	@echo 'Release fast gate PASSED: fmt clean + core/geometry/mesh/quality/refine_planner/project green.'
	@echo 'Full gate (needs NetCDF): make test-full'

check-method-c-neighbors:
	bash rust/earthmesh_mesh/scripts/check-method-c-neighbors.sh

clean:
	$(CARGO) clean --manifest-path $(CLI_MANIFEST)
	rm -f $(EXECUTABLE) logmake logmake_gnu logmake_rust *.o *.mod
	@echo 'Clean complete.'
