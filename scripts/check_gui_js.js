#!/usr/bin/env node
"use strict";

const fs = require("fs");

const read = (path) => fs.readFileSync(path, "utf8");
const html = read("gui-tauri/dist/index.html");
const readme = read("gui-tauri/README.md");
const capability = read("gui-tauri/src-tauri/capabilities/default.json");
const fileCommands = read("gui-tauri/src-tauri/src/file_commands.rs");
const libRs = read("gui-tauri/src-tauri/src/lib.rs");
const gitignore = read(".gitignore");
const tauriConfig = JSON.parse(read("gui-tauri/src-tauri/tauri.conf.json"));
const csp = tauriConfig.app.security.csp;
const maplibreJs = read("gui-tauri/dist/vendor/maplibre/maplibre-gl-csp.js");

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
  html.includes('href="vendor/openlayers/ol.css"') &&
    html.includes('src="vendor/openlayers/ol.js"') &&
    html.includes('href="vendor/maplibre/maplibre-gl.css"') &&
    html.includes('src="vendor/maplibre/maplibre-gl-csp.js"') &&
    html.includes('maplibregl.setWorkerUrl(new URL("vendor/maplibre/maplibre-gl-csp-worker.js",document.baseURI).href)') &&
    !html.toLowerCase().includes("leaflet") &&
    !html.includes("unpkg.com") &&
    !/<(?:script|link)[^>]+(?:src|href)=["']https?:/i.test(html) &&
    fs.existsSync("gui-tauri/dist/vendor/openlayers/ol.css") &&
    fs.existsSync("gui-tauri/dist/vendor/openlayers/ol.js") &&
    fs.existsSync("gui-tauri/dist/vendor/openlayers/LICENSE.md") &&
    fs.existsSync("gui-tauri/dist/vendor/maplibre/maplibre-gl.css") &&
    fs.existsSync("gui-tauri/dist/vendor/maplibre/maplibre-gl-csp.js") &&
    fs.existsSync("gui-tauri/dist/vendor/maplibre/maplibre-gl-csp-worker.js") &&
    fs.existsSync("gui-tauri/dist/vendor/maplibre/LICENSE.txt") &&
    maplibreJs.includes("v5.24.0") &&
    gitignore.includes("!gui-tauri/dist/vendor/openlayers/**") &&
    gitignore.includes("!gui-tauri/dist/vendor/maplibre/**") &&
    !gitignore.includes("!gui-tauri/dist/vendor/leaflet/**"),
  "OpenLayers and MapLibre GL JS 5.24.0 must be locally vendored and survive a clean checkout",
);
log("OpenLayers and MapLibre GL JS 5.24.0 are locally vendored");

check(
  csp["default-src"] === "'self'" &&
    csp["script-src"] === "'self'" &&
    csp["worker-src"] === "'self'" &&
    !csp["script-src"].includes("unsafe-inline") &&
    !csp["worker-src"].includes("blob:") &&
    csp["connect-src"].includes("https://server.arcgisonline.com") &&
    csp["img-src"].includes("https://server.arcgisonline.com"),
  "the self-hosted MapLibre CSP bundle and worker must run under a strict Tauri CSP",
);
log("map runtimes and CSP worker stay self-hosted under strict CSP");

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
    html.includes("thresholdRefine.enabled && (hasEnabledThresholdLayer(summary) || hasEnabledHydroRefinement(summary))") &&
    html.includes("thresholdEnabled: !!thresholdRefine.enabled") &&
    html.includes("let thresholdRefine = { enabled: false }") &&
    html.includes("thresholdRefine = { enabled: !!sum.threshold_refine_enabled }") &&
    html.includes('(l.role_kind === "threshold" || l.role_kind === "landcover")') &&
    !html.includes("landcoverCanRefine"),
  "threshold refinement must have an independent persisted master switch",
);
log("threshold refinement master switch is wired");

check(
  html.includes("const criterionEdits = {};") &&
    html.includes('invoke("set_threshold_criterion"') &&
    html.includes("sum.threshold_criteria") &&
    html.includes("criterion.source_id === l.id && criterion.enabled") &&
    html.includes("const sourceCriteria = cat.filter((criterion) => criterionStates[criterion.id] && criterionStates[criterion.id].source_id === l.id);") &&
    html.includes("sourceCriteria.map((criterion) => ({ ...l, id: criterion.id, sourceId: l.id, criterion") &&
    html.includes('row.dataset.isCriterion = l.isCriterion ? "1" : "0";') &&
    html.includes('if (row.dataset.isCriterion === "1")') &&
    html.includes("criterionEdits[id] = { enabled: next, value:") &&
    html.includes("criterionEdits[row.dataset.crit] = { enabled:"),
  "continuous threshold sources must render independent mean/std criteria with one shared path",
);
log("continuous thresholds expose independent mean/std criteria over one source path");

check(
  html.includes("const state = criterionStates.landcover;") &&
    html.includes('const criterion = cat.find((candidate) => candidate.id === "landcover");') &&
    html.includes("sourceEnabled: l.enabled, enabled: state.enabled, value: state.value") &&
    !html.includes('if (l.role_kind === "landcover") return true;'),
  "landcover refinement must be an independent categorical criterion, not the mask source toggle",
);
log("landcover criterion is independent from the mask source toggle");

check(
  html.includes('id: "hydroRiverWidth"') &&
    html.includes('id: "hydroRiverUpstreamArea"') &&
    html.includes('id: "hydroCoastDistance"') &&
    html.includes('label: z ? "河道细化 · MERIT"') &&
    html.includes('physical_process: z ? "河宽 ≥"') &&
    html.includes('physical_process: z ? "上游汇水面积 ≥"') &&
    html.includes('physical_process: z ? "距海岸线 ≤"') &&
    html.includes("const refinementCriteria = sum.layers.flatMap") &&
    html.includes('if (l.role_kind === "landcover") {') &&
    html.includes("const state = criterionStates.landcover;") &&
    html.includes('if (l.role_kind === "threshold") {') &&
    html.includes('if (l.role_kind === "merit") return hydroCriteria;') &&
    html.includes("refinementCriteria.forEach") &&
    !html.includes("[...crits, ...hydroCriteria]") &&
    html.includes('sides.className = "select em-hydro-sides"') &&
    html.includes('hydroRefine.coastEnabled = sides.value !== "none";') &&
    html.includes('hydroKey === "coastEnabled" && next && !hydroRefine.coastLandEnabled') &&
    html.includes('invoke("set_hydro_refinement"') &&
    html.includes("riverWidthEnabled: !!hydroRefine.riverWidthEnabled") &&
    html.includes("riverUpstreamAreaEnabled: !!hydroRefine.riverUpstreamAreaEnabled") &&
    html.includes("riverWidthThresholdM: hydroRefine.riverWidthThresholdM") &&
    html.includes("riverUpstreamAreaThresholdKm2: hydroRefine.riverUpstreamAreaThresholdKm2") &&
    html.includes("coastBufferKm: hydroRefine.coastBufferKm") &&
    html.includes("coastLandEnabled: !!hydroRefine.coastLandEnabled") &&
    html.includes("coastOceanEnabled: !!hydroRefine.coastOceanEnabled") &&
    html.includes("hydro_river_width_refine_enabled") &&
    html.includes("hydro_river_upstream_area_refine_enabled") &&
    html.includes("hydro_river_width_threshold_m") &&
    html.includes("hydro_river_upstream_area_threshold_km2") &&
    html.includes("const hasHydro = sum.hydro_coast_buffer_km != null || sum.hydro_river_width_threshold_m != null") &&
    !html.includes('id="hydroThresholdPanel"') &&
    !html.includes('id="hydroR2Width"') &&
    !html.includes('id="hydroR3Width"') &&
    readme.includes("riverWidthEnabled, riverUpstreamAreaEnabled") &&
    readme.includes("riverWidthThresholdM, riverUpstreamAreaThresholdKm2") &&
    readme.includes("hydro_river_width_refine_enabled") &&
    readme.includes("hydro_river_upstream_area_refine_enabled"),
  "MERIT-Hydro width, upstream area, and coast distance must be flat independent threshold rows",
);
log("MERIT-Hydro refinement criteria are flat independent rows");

check(
  html.includes("const h=_hydroThresholds;") &&
    html.includes("h.r3WidthM") &&
    html.includes("h.r3UpaKm2") &&
    html.includes("distance-refinement band is not shown") &&
    !html.includes("C2") &&
    !html.includes("C3") &&
    !html.includes("..._hydroThresholds") &&
    !html.includes("R3: 宽≥300m/上游≥5万km²") &&
    !html.includes("R2: width≥50m/upstream≥5k km²"),
  "MERIT map legend must follow the configured thresholds",
);
log("MERIT map legend follows project thresholds");

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
  html.includes('id="refinementAlgorithmPanel"') &&
    html.includes('id="refineBackendFamily"') &&
    html.includes('id="methodCAlgorithm"') &&
    html.includes('value="lepp_delaunay"') &&
    html.includes("LEPP-Delaunay / AdaptiveHybrid") &&
    html.includes('algorithmFamily = algorithm === "method_c" || algorithm === "lepp_delaunay" ? "method_c" : algorithm') &&
    html.includes("sum.refinement_algorithm || sum.refinement_backend") &&
    html.includes("+ algorithmBlock") &&
    !html.includes('<div id="refinementAlgorithmPanel" class="expert"'),
  "Method-C must visibly own Canonical and LEPP-Delaunay while Red-Green and HARP-DV remain independent backends",
);
log("algorithm hierarchy shows LEPP-Delaunay AdaptiveHybrid under Method-C");

check(
  html.includes('id="canonicalMethodCOptions"') &&
    html.includes('id="leppDelaunayOptions"') &&
    html.includes('id="redGreenOptions"') &&
    html.includes('id="harpDvOptions"') &&
    html.includes("const algorithmOptionsBlock = {") &&
    html.includes("+ algorithmOptionsBlock") &&
    html.includes('id="leppMaximumPathLength"') &&
    html.includes('id="harpMaximumPatchCells"') &&
    html.includes('invoke("set_method_c_algorithm_options"') &&
    html.includes('invoke("set_harp_dv_options"'),
  "the selected algorithm must be the only one whose complete production controls are rendered and saved",
);
log("algorithm-specific parameter panels are conditional and wired to Rust");

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
    html.includes('const map=ensureOlMap("mapsvgModal")') &&
    html.includes('map._resizeObserver=new ResizeObserver(scheduleMapResize)') &&
    html.includes('updateOlMap(map, !!payload.fit)') &&
    html.includes('grid-template-rows:auto minmax(0,1fr)') &&
    html.includes('body.map-window #mapStage{height:100%!important;min-height:0!important') &&
    html.includes('<div id="mapStage">') &&
    html.includes('<div id="mapglobeModal" class="earthmesh-globe" hidden></div>') &&
    capability.includes('"map"') &&
    capability.includes('"core:webview:allow-create-webview-window"'),
  "the enlarged map must open a state-synchronized Tauri window",
);
log("enlarged map opens in a native Tauri window");

check(
    html.includes('const map=ensureOlMap("mapsvg")') &&
    html.includes("new ol.layer.VectorImage") &&
    html.includes("featureClass:ol.render.Feature") &&
    html.includes('mesh=classified?_coastalGeojson:_meshGeojson') &&
    html.includes("map._geoRefs[key]===geojson && map._geoProjection[key]===cacheKey") &&
    html.includes("usableOlExtent(map._meshSource.getExtent())") &&
    !html.includes("getFeatures().length") &&
    !html.includes("_lmap") &&
    !html.includes("window.LEAF"),
  "embedded and planar maps must use one cached OpenLayers mesh source without raw/classified double drawing",
);
log("OpenLayers planar rendering avoids duplicate and unchanged GeoJSON work");

check(
  html.includes('id="mapRendererSelect"') &&
    html.includes('<option value="plane" data-i18n="map.renderer.plane">') &&
    html.includes('<option value="globe" data-i18n="map.renderer.globe">') &&
    html.includes('<option value="GLOBE" data-i18n="map.projection.globe" disabled>') &&
    html.includes('function setMapRenderer(map,renderer,doFit=true)') &&
    html.includes('renderer=renderer==="globe"?"globe":"plane"') &&
    html.includes('projection:{type:"vertical-perspective"}') &&
    html.includes('new maplibregl.Map({container,style:globeStyle()') &&
    html.includes('trackResize:false,canvasContextAttributes:{preserveDrawingBuffer:true}') &&
    !html.includes('canvasContextAttributes:{preserveDrawingBuffer:true,antialias:true}') &&
    html.includes('map._globeGeoRefs[key]===data') &&
    html.includes('map._globeGeoRefs[key]=data; source.setData(globeGeojson(data))') &&
    html.includes('while(longitude-previous>180) longitude-=360') &&
    html.includes('const mesh=hasGeojson(_coastalGeojson)?_coastalGeojson:_meshGeojson') &&
    !html.includes('globe.setStyle(') &&
    !html.includes('map._globe.setStyle('),
  "the independent map must switch to a fixed vertical-perspective globe without rebuilding unchanged raw GeoJSON",
);
log("globe rendering preserves raw GeoJSON identities and updates existing MapLibre sources");

check(
  html.includes('const ALL_MAP_STATE=["mesh","domain","coastal","settings"]') &&
    html.includes('if(selected.has("mesh")) payload.mesh=_meshGeojson') &&
    html.includes('if ("mesh" in payload) _meshGeojson = payload.mesh') &&
    html.includes('syncMapWindow(["settings"])') &&
    html.includes('syncMapWindow(["mesh","coastal"],true)') &&
    html.includes('publishMapState(["settings"],false)'),
  "map-window IPC must patch only changed fields instead of repeatedly cloning all GeoJSON",
);
log("map-window IPC preserves unchanged GeoJSON object identities");

check(
  html.includes('value="EPSG:3857"') &&
    html.includes('value="EPSG:4326"') &&
    html.includes('<option value="UTM:AUTO">') &&
    html.includes('<option value="streets"') &&
    html.includes('<option value="light"') &&
    html.includes('streets:{url:') &&
    html.includes('light:{url:') &&
    html.includes('function olUtmZone(lon,lat)') &&
    html.includes('function autoOlUtmCode(map)') &&
    html.includes('function olAutoUtmAvailable(') &&
    html.includes('resolveOlProjectionChoice(map,choice)') &&
    html.includes('function currentOlDomainFrame()') &&
    html.includes('frame&&frame.crossesDateline') &&
    html.includes('input[i]<west?input[i]+360:input[i]') &&
    html.includes('fitOlMap(map,scope,0,[width,height],null)') &&
    html.includes('canvas.toBlob(resolve,"image/png")') &&
    html.includes('function waitGlobeIdle(globe,timeoutMs=45000)') &&
    html.includes('function composeGlobeCanvas(map,width,height,contain=false)') &&
    html.includes('async function saveGlobeMapPng(map)') &&
    html.includes('if(map._globeActive) return saveGlobeMapPng(map)') &&
    html.includes('await waitGlobeIdle(globe)') &&
    html.includes('composeGlobeCanvas(map,width,height,scope==="view").toBlob(resolve,"image/png")') &&
    html.includes('pitch:globe.getPitch()') &&
    html.includes('if(scope==="view") globe.setPixelRatio(Math.min(width/viewRect.width,height/viewRect.height))') &&
    html.includes('if(scope==="view") globe.setPixelRatio(undefined)') &&
    html.includes('async function persistMapPng(blob)') &&
    html.includes('core.invoke("save_map_png",bytes)') &&
    html.includes('target.style.setProperty("width",width+"px","important")') &&
    html.includes('EarthMesh Studio · ${credit}') &&
    fileCommands.includes("tauri::ipc::InvokeBody::Raw") &&
    fileCommands.includes("validate_png_bytes(bytes)?") &&
    libRs.includes("save_map_png,"),
  "planar projection, antimeridian handling, both exact-size PNG exports, attribution, and raw native save must remain wired",
);
log("planar and globe PNG export contracts are wired");

check(
  [
    "mapWorldBtn",
    "mapRendererSelect",
    "mapMeshVisible",
    "mapBoundaryVisible",
    "mapDomainVisible",
    "mapGraticuleVisible",
    "mapLegendVisible",
    "mapBaseOpacity",
    "mapOpacity",
    "mapMeasureMode",
    "mapMeasureClearBtn",
  ].every((id) => html.includes(`id="${id}"`)) &&
    html.includes('fitOlMap(map,"global",300') &&
    html.includes('layer.setVisible(el.checked)') &&
    html.includes('map._baseLayer.setOpacity(value)') &&
    html.includes('map._meshLayer.setOpacity(value)') &&
    html.includes('setMapRenderer(map,renderer.value,true)') &&
    html.includes('setGlobeLayerVisible(map,key,el.checked)') &&
    html.includes('syncGlobePaint(map)') &&
    html.includes('setOlMeasureMode(map,measure.value)') &&
    html.includes('clearOlMeasurements(map)') &&
    html.includes('map._globeControlStates=map._globeContainer?Array.from') &&
    html.includes('map._globeControlStates.forEach(([element])=>{ element.disabled=true; })') &&
    html.includes('.earthmesh-globe .maplibregl-canvas:focus-visible') &&
    html.includes('map._basemapSources=map._basemapSources||{}'),
  "map exploration controls must update existing renderer objects instead of rebuilding mesh data",
);
log("map exploration controls preserve existing OpenLayers and MapLibre sources");

check(
  html.includes("function updateOlLegend(map)") &&
    html.includes('map._meshLayer.getFeatures(event.pixel)') &&
    html.includes("meshFeatureLabel(feature.getProperties())") &&
    html.includes('globe.queryRenderedFeatures(event.point,{layers:["earthmesh-mesh-fill"]})') &&
    html.includes('showCellInspectorProperties(map,features[0]&&features[0].properties)') &&
    html.includes('tooltip.className="ol-cell-tooltip"') &&
    html.includes('addEventListener("pointerleave"'),
  "both renderers must preserve the hydro legend and cell inspection",
);
log("OpenLayers and MapLibre preserve legend and cell inspection");

check(
  html.includes('lang=b.dataset.lang==="zh"?1:0; applyI18n();};') &&
    html.includes('}else{\n  applyI18n(); setupSplitters();') &&
    !html.includes('renderSteps(); renderStep(cur); applyI18n();'),
  "startup and language switches must render the workflow/map only once",
);
log("startup and language switching avoid duplicate renders");

{
  const splitters = section(
    html,
    /function setupSplitters\(\)\{([\s\S]*?)\n\}\n\nif\(MAP_WINDOW_MODE\)/,
    "splitter setup",
  );
  check(
    splitters.includes("scheduleMapResize()") && !splitters.includes("drawMap()"),
    "dragging splitters must resize maps without rebuilding GeoJSON layers",
  );
}
log("splitter dragging only resizes existing maps");

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
      body.includes("capabilities.target_presets") &&
      body.includes("capabilities.target_compatibility") &&
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
  html.includes('id="targetKindOutput"') &&
    html.includes('id="targetModelOutput"') &&
    !html.includes('id="targetModelOutput" value="—" readonly') &&
    html.includes('invoke("set_project_target"') &&
    html.includes("targetEdit = { kind:") &&
    // Model-to-cell capability, not kind-to-model: every model stays
    // selectable and the delivery field is what states the cost.
    html.includes("SPECIALIZED_CELLS") &&
    html.includes('id="targetDeliveryOutput"') &&
    html.includes("sum.target_kind") &&
    html.includes("sum.model_format"),
  "target kind/model must be editable canonical ProjectConfig state",
);
log("target kind/model are editable canonical state");

{
  const compose = section(html, /async function composeYaml\(\) \{([\s\S]*?)\n  \}/, "composeYaml body");
  const reflect = section(html, /async function reflectProject\(res\) \{([\s\S]*?)\n  \}/, "reflectProject body");
  const wire = section(html, /async function wireExpertTargetStep\(\) \{([\s\S]*?)\n  \}/, "wireExpertTargetStep body");
  check(
    compose.includes("yaml, nxp: expertEdit.nxp") &&
      compose.includes("halo: expertEdit.halo") &&
      compose.includes("maxTransitionRow: expertEdit.maxTransitionRow") &&
      compose.includes("weakConcavEliminate: expertEdit.weakConcavEliminate") &&
      reflect.includes("nxp: sum.expert_nxp ?? null") &&
      reflect.includes("weakConcavEliminate: sum.expert_weak_concav_eliminate ?? null") &&
      wire.includes("nxp: expertEdit.nxp") &&
      wire.includes("weakConcavEliminate: expertEdit.weakConcavEliminate") &&
      !compose.includes("nxp: null, openmp:") &&
      !compose.includes("weakConcavEliminate: discreteMask ? true : null"),
    "open-compose-save must preserve hidden expert overrides exactly",
  );
  check(
    compose.includes("Object.keys(layerEdits).sort((a,b) => Number(!!layerEdits[b].enabled) - Number(!!layerEdits[a].enabled))") &&
      compose.includes('yaml = await invoke("set_layer_path"') &&
      compose.includes('yaml = await invoke("set_threshold_value"') &&
      compose.includes('yaml = await invoke("set_threshold_criterion"') &&
      !compose.includes("catch (err)"),
    "compose must surface data-layer and criterion validation errors",
  );
  check(
    compose.indexOf('invoke("set_adaptive_refinement"') < compose.indexOf('invoke("set_refinement_backend"') &&
      compose.indexOf('invoke("set_hfield_refinement"') < compose.indexOf('invoke("set_refinement_backend"') &&
      compose.includes('invoke("preserve_unexposed_quality_fields"') &&
      reflect.includes('algorithm: sum.refinement_algorithm || sum.refinement_backend || "method_c"'),
    "opened GUI projects must restore backend choice after route setters and preserve hidden LEPP quality only after compatibility is known",
  );
  log("opened project backend/route/hidden-LEPP round-trip is ordered safely");
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

{
  const reset = section(
    html,
    /window\.resetTemplateDerivedState = function \(\) \{([\s\S]*?)\n  \};/,
    "resetTemplateDerivedState body",
  );
  check(
    html.includes("function selectTemplate(k)") &&
      reset.includes("targetEdit = null;") &&
      !reset.includes("delete layerEdits[id]") &&
      !reset.includes("delete thresholdEdits[id]") &&
      !reset.includes("baseProjectYaml = null") &&
      !reset.includes("qualityEdit = null") &&
      !reset.includes("thresholdRefine = { enabled: false }") &&
      html.includes("cur = 1;") &&
      html.includes("c.onclick=()=>selectTemplate(+c.dataset.tpl)"),
    "template switch must apply a one-shot target preset without clearing common project edits",
  );
  log("template switch preserves common project state");
}

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
  readme.includes("target_kind") &&
    readme.includes("threshold_criteria:[{id,source_id,statistic,source_enabled,enabled,value}]") &&
    readme.includes("layers:[{id,role_kind,source_field,role,path,enabled,threshold_value,wants_folder}]"),
  "project_summary README must document target, criterion, and layer shapes",
);
log("project_summary target/criterion/layer shapes documented");

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
  // Method-C was once offered as a project output format and is not one; that
  // is what this bans. It *is* one of the two refinement algorithms, and the
  // picker that chooses between them has to say its name, so the picker's own
  // markup is taken out before the check rather than the check being dropped.
  // The route picker has to name it too: only Method-C serves the h-field, and
  // an option greyed out with no reason given is worse than one that says why.
  const withoutAlgorithmPicker = (text) =>
    text
      .replace(/\$\{field\([^]*?id="refineAlgorithm"[^]*?\)\}/g, "")
      .replace(/\$\{field\([^]*?id="refineBackend"[^]*?\)\}/g, "");
  const hits = [
    ["gui-tauri/README.md", readme],
    ["gui-tauri/dist/index.html", html],
  ]
    .filter(([, text]) => /\bMethod-C\b/.test(withoutAlgorithmPicker(text)))
    .map(([file]) => file);
  check(!hits.length, "GUI/docs must not expose Method-C as a project output format", hits);
  log("GUI/docs hide deprecated Method-C project output; the algorithm picker may name it");
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
    html.includes('tr.dataset.sourceField = l.source_field || "";') &&
    html.includes("const selectExclusiveSource = (id, path) => {") &&
    html.includes("sibling.source_field === selected.source_field") &&
    html.includes('layerEdits[sibling.id] = { path: sibling.path || "", enabled: false };') &&
    html.includes("if (enabled) selectExclusiveSource(id, path);") &&
    !html.includes("const e = layerEdits[id];\n        if (!e || !e.path) return;"),
  "layer toggles must preserve paths and keep same-field sources exclusive",
);
log("layer toggles preserve paths and keep same-field sources exclusive");

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
      body.includes('const mode = (s && s.quality_mode) || (cell === "tri" ? "tri-strict" : "hex-cgrid");') &&
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
    html.includes("const refinementEnabled = (thresholdRefine.enabled && (hasEnabledThresholdLayer(sum) || hasEnabledHydroRefinement(sum))) || !!specifiedRefine.enabled;") &&
      !html.includes("const refinementEnabled = regionalRefine ||") &&
      !html.includes("regionalAutoPasses") &&
      html.includes("const shownPasses") &&
      html.includes("no threshold-capable data layers") &&
      html.includes('anchor.insertAdjacentHTML("afterend", mp);') &&
      html.includes(
        "const refinementPasses = refinementEnabled",
      ),
    "disabled refinement max_passes must stay zero/inert",
  );
  log("disabled refinement max_passes zero check passed");
}

check(
  html.includes('hydroRefine[hydroThresholdKey] = raw === ""') &&
    html.includes("? defaultHydroRefine()[hydroThresholdKey]") &&
    html.includes(": Number.isFinite(v) ? v : 0;") &&
    !html.includes("if (Number.isFinite(v) && v > 0) {\n            hydroRefine[hydroThresholdKey] = v;"),
  "blank MERIT thresholds must restore defaults while invalid values reach Rust validation",
);
log("blank MERIT thresholds restore defaults; invalid values reach Rust validation");

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
    body.includes("label.textContent = isCriterion && z") &&
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
      body.includes('row.dataset.enabled = criterionEnabled ? "1" : "0";') &&
      body.includes('layerEdits[id] = { path, enabled: row.dataset.enabled !== "1" };') &&
      body.includes('criterionEdits[id] = { enabled: next, value:') &&
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
  // Three expert controls are parsed from the namelist, validated, lowered and
  // written back -- and read by no refinement code. Measured on all three
  // backends with the relevant spring proven to be running: changing any of
  // them leaves the mesh bit-identical. A number a user can set that does
  // nothing has to say so, so each carries the reason in its help text. If one
  // is ever implemented, that sentence is what has to come back out.
  const inert = ["RL%set_dis_type", "RL%num_rc", "RL%vertex_pretect_layers"];
  const missing = inert.filter((name) => {
    const at = html.indexOf(`\${field("${name}"`);
    if (at < 0) return true;
    return !html
      .slice(at, at + 900)
      .includes("does not affect the mesh in this build");
  });
  check(
    !missing.length,
    "expert controls that no backend reads must say so in their help text",
    missing,
  );
  log("inert expert controls are labelled as carried-for-fidelity");
}

{
  // Algorithm and route were two selects that knew nothing about each other, so
  // the pair `harp_dv` + h-field was one click away and the run refuses it.
  // Both halves are needed: disabling the option stops it being chosen, and the
  // reset stops it staying chosen, because a browser keeps a disabled option
  // selected when it already was.
  check(
    /id="refineBackend"[^]*?value="hfield"[^]*?\$\{hfieldServed\?"":"disabled"\}/.test(html),
    "the h-field route option must be disabled for a backend that cannot serve it",
  );
  check(
    html.includes('specifiedRefine.algorithm !== "method_c" && (specifiedRefine.route || "adaptive") === "hfield"'),
    "switching algorithm must reset a selected h-field route",
  );
  log("h-field route is gated on the refinement algorithm");
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
