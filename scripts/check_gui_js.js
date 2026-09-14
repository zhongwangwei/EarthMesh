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
check(!/harp[_-]?dv|harp[A-Z]/i.test(html), "retired HARP controls must not be exposed");
check(!libRs.includes("set_harp_dv_options"), "retired HARP command must not be registered");
log("retired HARP UI and commands are absent");

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
    html.includes("blank defaults to ${DEFAULT_SURFACE_REFINE_SPRING_ITERATIONS} for surface") &&
    html.includes("DEFAULT_ATMOSPHERE_REFINE_SPRING_ITERATIONS"),
  "niter_refine must remain unset unless the user explicitly overrides it",
);
log("niter_refine default remains engine-owned");

check(
  html.includes('const springControls = algorithm === "certified"') &&
    html.includes("generic spring smoothing would invalidate the certificate.") &&
    html.includes('${springControls}'),
  "CMRC must explain and hide the inapplicable generic spring controls",
);
log("CMRC hides inapplicable generic spring controls");

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
  html.includes('if (l.role_kind === "landcover" || l.role_kind === "threshold") {') &&
    html.includes("criterionStates[criterion.id].source_id === l.id") &&
    html.includes("isCriterion: true, sourceEnabled: l.enabled") &&
    !html.includes('if (l.role_kind === "landcover") return true;'),
  "landcover refinement must be an independent categorical criterion, not the mask source toggle",
);
log("landcover criterion is independent from the mask source toggle");

{
  // Execute the actual source-to-row mapping, including custom LandType ids.
  const body = section(html, /const refinementCriteria = sum.layers.flatMap\(\(l\) => \{([\s\S]*?)\n    \}\);/, "refinement row mapping");
  const rowsFor = new Function("sum", "cat", "criterionStates", "hydroCriteria", `return sum.layers.flatMap((l) => {${body}\n});`);
  const cat = [{ id: "landcover" }, { id: "sea_ratio" }, { id: "lai_mean" }];
  const criterionStates = {
    landcover: { source_id: "custom_mask", enabled: false, value: 12 },
    sea_ratio: { source_id: "custom_mask", enabled: true, value: 0.05 },
    lai_mean: { source_id: "lai", enabled: true, value: 1 },
  };
  const source = { id: "custom_mask", role_kind: "landcover", path: "/data/mask.nc", enabled: true };
  const sum = { layers: [{ ...source, id: "unused_mask", enabled: false }, source, { id: "merit", role_kind: "merit" }] };
  const rows = rowsFor(sum, cat, criterionStates, [{ id: "hydroCoastDistance" }]);
  check(JSON.stringify(rows.map((row) => row.id)) === JSON.stringify(["landcover", "sea_ratio", "hydroCoastDistance"]), "LandType must render both criteria once and keep MERIT separate");
  check(!rows[0].enabled && rows[1].enabled && rows[1].value === 0.05 && rows[1].path === source.path, "land/sea criterion must preserve its independent state on the shared source");
  source.enabled = false;
  check(rowsFor(sum, cat, criterionStates, [])[1].sourceEnabled === false, "disabling LandType must disable availability, not the sea-ratio criterion state");
  check(html.includes('"海陆分布"') && html.includes('"aria-checked", String(on)') && html.includes('tog.disabled = !hasData || !thresholdRefine.enabled;'), "land/sea row must be localized and keyboard accessible");
  log("land/sea and landcover rows share one source with independent state");
}

check(
  html.includes('id: "hydroRiverWidth"') &&
    html.includes('id: "hydroRiverUpstreamArea"') &&
    html.includes('id: "hydroCoastDistance"') &&
    html.includes('label: z ? "河道细化 · MERIT"') &&
    html.includes('physical_process: z ? "河宽 ≥"') &&
    html.includes('physical_process: z ? "上游汇水面积 ≥"') &&
    html.includes('physical_process: z ? "距海岸线 ≤"') &&
    html.includes("const refinementCriteria = sum.layers.flatMap") &&
    html.includes('if (l.role_kind === "landcover" || l.role_kind === "threshold") {') &&
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
    html.includes('value="certified"') &&
    html.includes("LEPP-Delaunay / AdaptiveHybrid") &&
    html.includes('algorithmFamily = algorithm === "method_c" || algorithm === "lepp_delaunay" ? "method_c" : algorithm') &&
    html.includes('${algorithmFamily==="method_c"?`<div id="methodCAlgorithmChoice">') &&
    !html.includes('id="methodCAlgorithmChoice" style=') &&
    html.includes("sum.refinement_algorithm || sum.refinement_backend") &&
    html.includes("+ algorithmBlock") &&
    !html.includes('<div id="refinementAlgorithmPanel" class="expert"'),
  "Method-C must visibly own Canonical and LEPP-Delaunay while CMRC and Red-Green remain peer backends",
);
log("algorithm hierarchy shows LEPP-Delaunay AdaptiveHybrid under Method-C");

check(
  html.includes('id="canonicalMethodCOptions"') &&
    html.includes('id="leppDelaunayOptions"') &&
    html.includes('id="redGreenOptions"') &&
    html.includes('id="certifiedOptions"') &&
    html.includes("const algorithmOptionsBlock = {") &&
    html.includes("+ algorithmOptionsBlock") &&
    html.includes('id="leppMaximumPathLength"') &&
    html.includes('invoke("set_method_c_algorithm_options"') &&
    html.includes('invoke("set_certified_options"'),
  "the selected algorithm must be the only one whose complete production controls are rendered and saved",
);
log("algorithm-specific parameter panels are conditional and wired to Rust");

check(
  html.includes('certified_defaults:{mode:"reverse_coarsening"') &&
    html.includes('angle_contract:"domain_quality_38_to_82_v1"') &&
    html.includes('function meshPreviewStride(totalCells, bbox)') &&
    html.includes('cellStride:stride, bbox') &&
    html.includes('map.on("moveend",()=>scheduleMeshViewportPreview(map))') &&
    html.includes('scheduleMeshViewportPreview = function (map, force=false)') &&
    html.includes('if (previewChanged && _meshPreview) scheduleMeshViewportPreview(map, true)') &&
    html.indexOf('const openButton = (label, path) =>') < html.indexOf('function renderCertifiedRun(bundle)') &&
    (html.match(/const openButton =/g) || []).length === 1 &&
    !html.includes('function loadCompleteMeshPolygons('),
  "CMRC must default to adaptive reverse coarsening and large meshes must use full-extent viewport LOD",
);
log("CMRC defaults to reverse coarsening and large meshes use viewport LOD");

{
  const canonical = section(html, /const canonicalMethodCOptions = `([\s\S]*?)`;\n    const leppOptions/, "Canonical Method-C options");
  const redGreen = section(html, /const redGreenOptions = `([\s\S]*?)`;\n    const certifiedOptions/, "Red-Green options");
  check(
    !canonical.includes('id="expertWeakConcav"') &&
      redGreen.includes('id="expertWeakConcav"') &&
      html.includes('if (weakConcav) expertEdit.weakConcavEliminate ='),
    "weak-concavity elimination must belong only to Red-Green while hidden values remain preserved",
  );
  log("weak-concavity control is Red-Green-only");
}

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
    html.includes('payload.meshPreview=_meshPreview&&{gridfile:_meshPreview.gridfile') &&
    html.includes('payload.mesh=_meshPreview?null:_meshGeojson') &&
    html.includes('if ("mesh" in payload) _meshGeojson = payload.mesh') &&
    html.includes('if ("meshPreview" in payload) _meshPreview = payload.meshPreview') &&
    html.includes('syncMapWindow(["settings"])') &&
    html.includes('syncMapWindow(["mesh","coastal"],!!doFit)') &&
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
      body.includes("capabilities.method_c_max_refinement_level") &&
      body.includes("capabilities.default_surface_refine_spring_iterations") &&
      body.includes("capabilities.default_atmosphere_refine_spring_iterations") &&
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

check(
  html.includes('id="colmMeshDeliveryControls"') &&
    html.includes('id="colmMeshEnabled"') &&
    html.includes('id="colmMeshPixelsPerDegree"') &&
    html.includes('invoke("set_colm_mesh_delivery"') &&
    html.includes("setColmMeshDelivery:") &&
    html.includes("selectedModelForDelivery === \"CoLM\"") &&
    html.includes("Number.isInteger(colmMeshDelivery.pixelsPerDegree)") &&
    html.includes('colmMeshDelivery = { enabled: !!summary.colm_mesh_enabled, pixelsPerDegree: summary.colm_mesh_pixels_per_degree || 240 }') &&
    libRs.includes("set_colm_mesh_delivery,"),
  "CoLM mesh delivery UI must be backed by a registered Tauri IPC command",
);
log("CoLM mesh delivery command is wired and model-gated");

{
  const compose = section(html, /async function composeYaml\([^)]*\) \{([\s\S]*?)\n  \}/, "composeYaml body");
  const reflect = section(html, /async function reflectProject\(res\) \{([\s\S]*?)\n  \}/, "reflectProject body");
  const wire = section(html, /async function wireExpertTargetStep\(\) \{([\s\S]*?)\n  \}/, "wireExpertTargetStep body");
  check(
    compose.includes("yaml, nxp: expertEdit.nxp") &&
      compose.includes("halo: expertEdit.halo") &&
      compose.includes("maxTransitionRow: expertEdit.maxTransitionRow") &&
      compose.includes("weakConcavEliminate: expertEdit.weakConcavEliminate") &&
      reflect.includes("nxp: sum.expert_nxp ?? null") &&
      reflect.includes("weakConcavEliminate: sum.expert_weak_concav_eliminate ?? null") &&
      reflect.includes("const algorithmDefaults = defaultAlgorithmControls();") &&
      wire.includes("nxp: expertEdit.nxp") &&
      wire.includes("weakConcavEliminate: expertEdit.weakConcavEliminate") &&
      wire.includes("isolatedOcean: expertEdit.isolatedOcean") &&
      !wire.includes("[spring,") &&
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
      compose.indexOf('invoke("set_refinement_backend"') < compose.indexOf('invoke("set_refinement"') &&
      compose.includes('invoke("preserve_unexposed_quality_fields"') &&
      reflect.includes('algorithm: sum.refinement_algorithm || sum.refinement_backend || "method_c"'),
    "opened GUI projects must configure routes and backend before enabling refinement, and preserve hidden LEPP quality only after compatibility is known",
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
      html.includes("preserveDomain: !template && !!hiddenDomainShape") &&
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
      html.includes("let runCompletion = null;") &&
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
    html.includes('commitProjectEdit(yaml => invoke("set_layer_path", { yaml, id, path, enabled }))') &&
    html.includes('await editLayer(id, p, true);') &&
    html.includes('await editLayer(id, "", false);') &&
    html.includes('await editLayer(id, path, enabled);') &&
    html.includes('commitProjectEdit(yaml => api.autofillLayers(yaml, folder))') &&
    !html.includes('selectExclusiveSource') &&
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
      body.includes("openButton(") &&
      html.includes('invoke("open_path", { path })') &&
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
    html.includes("METHOD_C_DEFAULTS = capabilities.method_c_defaults") &&
    html.includes("CERTIFIED_DEFAULTS = capabilities.certified_defaults") &&
    html.includes("const algorithmDefaults = defaultAlgorithmControls();") &&
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
  check(
    html.includes('id="qualityMinAngle"') &&
      body.includes('document.getElementById("qualityMinAngle")') &&
      body.includes("let minAngle = 0") &&
      !body.includes("let minAngle = 25"),
    "frontend quality min angle must use a stable control and pass invalid input to Rust validation",
  );
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

{
  check(
    html.includes("METHOD_C_MAX_REFINEMENT_LEVEL = capabilities.method_c_max_refinement_level"),
    "refinement controls must use the runtime schema limit",
  );
  const compose = section(html, /(const maxRefinePasses =[\s\S]*?\n      : 0;)/, "refinement compose limits");
  const controls = section(html, /(const nMax =[\s\S]*?const shownCalPasses =[^\n]*;)/, "refinement control limits");
  const legacyCap = html.match(/  function regionalMethodCLevelCap\(nxp\) \{[\s\S]*?\n  \}/)?.[0] || "";
  const probe = new Function("summary", "requested", "source", "algorithm", `
    const METHOD_C_MAX_REFINEMENT_LEVEL=5, METHOD_C_MIN_BASE_NXP=10;
    const currentNxp=()=>summary.effective_nxp;
    ${legacyCap}
    let maxPasses=requested;
    const expertEdit={};
    const thresholdRefine={enabled:source==="threshold"};
    const specifiedRefine={enabled:source==="specified",algorithm};
    const hasEnabledThresholdLayer=()=>source==="threshold";
    const hasEnabledHydroRefinement=()=>false;
    const refinementEnabled=source!=="off",template=null;
    ${compose}
    {
      const sum=summary, regionalRefine=sum.domain==="regional";
      ${controls}
      return {composed:refinementPasses,afterPaint:maxPasses,nMax,shownPasses,shownSpcPasses,shownCalPasses};
    }
  `);
  for (const algorithm of ["method_c", "red_green", "lepp_delaunay", "certified"])
    for (const domain of ["global", "regional"])
      for (const effective_nxp of [3, 40])
        for (const source of ["specified", "threshold", "off"])
          for (const requested of source === "off" ? [0, 3] : [1, 3, 5]) {
            const actual = probe({domain,effective_nxp}, requested, source, algorithm);
            check(actual.composed === (source === "off" ? 0 : requested) && actual.afterPaint === requested,
              "compose/paint must preserve requested depth; AutoRefine projection belongs to the CLI",
              {algorithm,domain,effective_nxp,source,requested,actual});
            check(actual.nMax === 5 && actual.shownSpcPasses === Math.max(1, requested) && actual.shownCalPasses === Math.max(1, requested),
              "pass controls must expose schema-valid requests without an NXP cap", actual);
          }
  log("refinement demand: actual compose/control snippets preserve requested depths and disabled zero across domains/algorithms");
}

{
  const body = section(html, /async function enhanceRefinementStep\(\) \{([\s\S]*?)\n  \}/, "enhanceRefinementStep body");
  check(
    body.includes('label.textContent = l.id === "sea_ratio" && z ? "海陆分布" : isCriterion && z') &&
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

check(
  html.includes('["off", z?"关闭弹性调整":"Disable spring smoothing"]') &&
    html.includes('if(strategy==="off") return {springGlobalType:0,springRegionalType:0};') &&
    html.includes('const expertRefine = `<div style="border:1px solid var(--border)') &&
    html.includes('+ algorithmOptionsBlock\n        + expertRefine') &&
    !html.includes('id="expertSpringStrategy"') &&
    !html.includes('(strategyEnabled ? hfieldBlock + expertRefine : "")'),
  "spring strategy and iterations must have one visible home for every applicable algorithm",
);
log("common spring controls stay visible for every applicable algorithm");

{
  // These are preserved from an opened project for namelist fidelity, but no
  // production refinement backend consumes them. Do not present inert knobs.
  const inert = ["RL%set_dis_type", "RL%num_rc", "RL%vertex_pretect_layers"];
  check(
    inert.every((name) => !html.includes(`\${field("${name}"`)) &&
      html.includes("setDisType: expertEdit.setDisType") &&
      html.includes("numRc: expertEdit.numRc") &&
      html.includes("vertexPretectLayers: expertEdit.vertexPretectLayers"),
    "inert compatibility values must be preserved without exposing dead controls",
  );
  log("inert compatibility values are preserved but not exposed");
}

check(
  html.includes('${backend==="discrete"?`<option value="discrete" selected>') &&
    !html.includes('<option value="discrete" ${backend==="discrete"?"selected":""}>'),
  "new projects must not expose the unconfigurable discrete-mask route",
);
log("discrete mask is existing-project-only");

{
  const body = section(html, /function expertEnabled\(\) \{([\s\S]*?)\n  \}/, "expertEnabled body");
  check(
    body.includes("expertEdit.nxp != null") &&
      body.includes("expertEdit.weakConcavEliminate != null") &&
      body.includes("expertEdit.isolatedOcean != null") &&
      !body.includes("discreteMask") &&
      html.includes("expertEdit.enabled = expertEnabled();"),
    "expert mode must reflect actual overrides instead of the demand route",
  );
  log("expert visibility follows actual overrides");
}

{
  // Algorithm and route were two selects that knew nothing about each other, so
  // an unsupported backend + h-field was one click away and the run refuses it.
  // Both halves are needed: the non-Method-C DOM must not contain H-field
  // controls, and stale projects must be reset before rendering or saving.
  check(
    html.includes('${hfieldServed?`<option value="hfield"') &&
      html.includes('${hfieldServed?`<div id="hfieldOptions"') &&
      html.includes('if (!hfieldServed && specifiedRefine.route === "hfield") specifiedRefine.route = "adaptive";'),
    "non-Method-C algorithms must not render or retain H-field controls",
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

// Execute the actual delivery renderer, including legacy/failed paths, without a DOM dependency.
{
  const body = section(html, /function renderProjectDelivery\(result\) \{([\s\S]*?)\n  \}\n\n  function renderCertifiedRun/, "actual delivery renderer");
  const render = new Function("result", "document", "zh", "openButton", body);
  const element = () => ({
    style: {}, children: [], value: "",
    set textContent(value) { this.value = value; this.children = []; },
    get textContent() { return this.value + this.children.map(child => child.textContent).join(" "); },
    append(...children) { this.children.push(...children); },
    appendChild(child) { this.append(child); },
  });
  for (const chinese of [false, true]) {
    const card = element(), links = [];
    const document = { getElementById: () => card, createElement: element };
    const open = (name, path) => { links.push(path); const button = element(); button.textContent = name; return button; };
    const report = { target: {kind:"Land",cell:"Tri",model_format:"CoLM"}, capability:"full",
      gridfile:"/run/final.nc4", final_quality:{report:"/run/quality.json",verdict:"warn"},
      model_delivery_status:"model_delivered", model_artifacts:{colm_mesh_input:"/run/colm.nc"}, skipped_reason:null };
    const result = {ok:true,delivery:{report_path:"/run/delivery.json",report}};
    render(result, document, () => chinese, open);
    check(card.textContent.includes(chinese ? "模型文件已交付" : "Model files delivered"), "actual delivered renderer");
    check(links.join() === "/run/final.nc4,/run/quality.json,/run/delivery.json,/run/colm.nc", "renderer must bind current record paths");
    report.model_delivery_status="native_only"; report.model_artifacts={}; report.skipped_reason="<img src=x>";
    links.length=0; render(result, document, () => chinese, open);
    check(card.textContent.includes(chinese ? "仅生成通用网格" : "Native mesh only") && card.textContent.includes("<img src=x>"), "native-only reason renders as text");
    check(links.length===3, "native-only must not expose model links");
    for (const input of [{ok:true}, {...result,ok:false}, null]) {
      links.length=0; render(input, document, () => chinese, open);
      check(links.length===0 && !card.textContent.includes("CoLM"), "failed or legacy runs must clear delivery links");
      check(card.textContent.includes(input && input.ok ? (chinese ? "无法确认" : "unconfirmed") : (chinese ? "运行失败" : "Run failed")), "unknown is not delivered");
    }
  }
  check(html.includes("delivery: r.ok ? r.delivery || null : null") && html.includes('deliveryCard.textContent = ""; deliveryCard.style.display = "none";'), "actual delivery is captured only on success and cleared on restart");
  log("actual delivery renderer: final links, native-only, legacy, failure and bilingual text passed");
}

// Run the real async loaders with controlled IPC promises: no browser/test dependency.
async function checkAnalysisOwnership() {
  const names = ["meshViewKind", "readMeshQuality", "loadMeshMeritCells", "loadMeshPreview", "loadQualityAndMesh"];
  const definitions = names.map(name => section(html,
    new RegExp(`  ((?:async )?function ${name}\\([^\\n]*\\) \\{[\\s\\S]*?\\n  \\})`), name));
  const harness = new Function(`
    let runInfo=null,lastSummary=null,_lastQuality=null,_meshPreview=null,_meshPreviewToken=0,_meshGeojson=null,_hydroThresholds;
    const events=[],pending=[],DEFAULT_MIN_ANGLE_DEG=25,MESH_VIEW_CELLS=50000,MERIT_SURFACE_PREVIEW_STRIDE=50;
    const invoke=(command,args)=>new Promise((resolve,reject)=>pending.push({command,args,resolve,reject}));
    const zh=()=>false,meshPreviewStride=()=>1,certifiedPreviewCellCount=()=>0;
    const meshMeritLayer=s=>s&&s.merit,meshLandcoverLayer=()=>null;
    const renderQualityCard=q=>events.push(['quality',q]),renderQualityNote=s=>events.push(['note',s]);
    const logLine=s=>events.push(['log',s]),clearCoastalOverlay=()=>events.push(['clear']);
    const applyCoastal=mesh=>events.push(['coastal',mesh]);
    const applyMesh=mesh=>{_meshGeojson=mesh;events.push(['mesh',mesh]);};
    ${definitions.join("\n")}
    return {events,pending,
      set(result,live){runInfo=result;lastSummary=live;_meshPreviewToken++;_lastQuality=null;_meshPreview=null;},
      load(known){return loadQualityAndMesh(runInfo.gridfile,known);},
      merit(){loadMeshMeritCells(runInfo.gridfile,meshViewKind(),runInfo);},
      view:meshViewKind,quality(){return _lastQuality;}};
  `);
  const take = (h, command) => {
    const index=h.pending.findIndex(p=>p.command===command);
    check(index>=0, `missing ${command}`);
    return h.pending.splice(index,1)[0];
  };
  const flush = () => new Promise(resolve=>setImmediate(resolve));
  const old = {gridfile:"/old.nc",summary:{cell:"tri",min_angle_deg:32,on_violation:"warn"}};
  const current = {gridfile:"/new.nc",summary:{cell:"hex",min_angle_deg:25,on_violation:"warn"}};
  const mesh = JSON.stringify({features:[{id:"current"}]});
  for (const outcome of ["resolve","reject"]) {
    const h=harness();h.set(old,current.summary);h.load();
    const delayed=take(h,"mesh_quality");
    check(delayed.args.kind==="tri" && delayed.args.minAngleDeg===32 && delayed.args.onViolation==="warn", "analysis must use selected run snapshot, not live summary");
    h.set(current,old.summary);h.load();
    take(h,"mesh_quality").resolve({cell_count:92});await flush();
    take(h,"mesh_cell_polygons").resolve(mesh);await flush();
    const before=JSON.stringify(h.events);
    delayed[outcome](outcome==="resolve"?{cell_count:180}:new Error("old quality"));await flush();
    check(h.quality().cell_count===92 && JSON.stringify(h.events)===before && h.pending.length===0, "old quality must not repaint, start preview or log errors");

    const p=harness();p.set(old);p.load({cell_count:180});
    const stale=take(p,"mesh_cell_polygons");
    p.set(current);p.load({cell_count:92});
    take(p,"mesh_cell_polygons").resolve(mesh);await flush();
    const previewBefore=JSON.stringify(p.events);
    stale[outcome](outcome==="resolve"?JSON.stringify({features:[{id:"old"}]}):new Error("old preview"));await flush();
    check(JSON.stringify(p.events)===previewBefore, "old preview must not repaint, classify or log errors");

    const c=harness();
    const withMerit=result=>({...result,summary:{...result.summary,merit:{path:"/merit"},bbox:[100,120,0,40]}});
    c.set(withMerit(old));c.merit();const oldCoast=take(c,"mesh_merit_cells");
    c.set(withMerit(current));c.merit();take(c,"mesh_merit_cells").resolve(mesh);await flush();
    check(c.events.some(e=>e[0]==="coastal"), "current coastal result must still apply");
    const coastBefore=JSON.stringify(c.events);
    oldCoast[outcome](outcome==="resolve"?JSON.stringify({features:[{id:"old coast"}]}):new Error("old coast"));await flush();
    check(JSON.stringify(c.events)===coastBefore, "old classification must not repaint, clear or log errors");
  }
  const h=harness();h.set({...current,delivery:{report:{target:{cell:"Tri"}}}},old.summary);
  check(h.view()==="tri", "actual delivered target must outrank the summary");
  h.load();take(h,"mesh_quality").reject(new Error("current quality"));await flush();
  check(h.events.some(e=>e[0]==="note" && e[1].includes("current quality")), "current analysis errors must remain visible");
  check(html.includes("summary: runSummary") && html.indexOf("runSummary = await api.summary(yaml)")<html.indexOf("const r = await api.runProject"), "snapshot run summary before awaiting engine completion");
  log("analysis ownership: stale quality/preview/coastal success and error suppressed; selected view/snapshot/current errors passed");
}
checkAnalysisOwnership().catch(error => { console.error(error); process.exitCode=1; });

// Exercise the actual run/stop handlers with delayed command completion.
async function checkRunSettlement() {
  const extract = (name, indent) => section(html, new RegExp(`${indent}((?:async )?function ${name}\\([^\\n]*\\) ?\\{[\\s\\S]*?\\n${indent}\\})`), name);
  const definitions = [extract("doRun", "  "), extract("enhanceRunStep", "  "), extract("setRunControls", ""), extract("killRun", ""), extract("confirmStopForPageSwitch", "")];
  const harness = new Function(`
    let runInProgress=false,runCompletion=null,killInProgress=false,hasRun=true,runInfo={ok:true,outdir:'/old'},_lastQuality=null,lastSummary=null,cur=6,lang=0,outputPath='';
    const pending=[],events=[],elements={};
    const element=()=>({textContent:'',style:{},classList:{toggle(){}},appendChild(node){events.push(['log',node.textContent]);}});
    for(const id of ['runBtn','killBtn','rtext','rdot','logbox'])elements[id]=element();
    const document={getElementById:id=>elements[id],querySelectorAll:()=>[],createElement:element};
    const defer=command=>new Promise((resolve,reject)=>pending.push({command,resolve,reject}));
    const invoke=command=>defer(command),window={__TAURI__:{core:{invoke}}};
    const api={summary:async()=>({cell:'tri'}),runProject:()=>defer('run_project')};
    let composeYaml=async()=>'yaml',projectEditQueue=Promise.resolve();
    const zh=()=>false,confirm=()=>true,currentIntent=()=>'',currentResolutionLabel=()=>'';
    const logLine=s=>events.push(['log',s]);
    const clearRunArtifacts=()=>{hasRun=false;runInfo=null;};
    const renderAutoRefineDecisions=()=>{},renderCertifiedRun=()=>{},renderProjectDelivery=()=>{},renderQualityCard=()=>{},renderMissingGridfile=()=>{};
    const loadQualityAndMesh=path=>events.push(['load',path]);
    const renderStep=()=>{events.push(['render',hasRun,runInfo]);elements.runBtn=element();elements.killBtn=element();enhanceRunStep();};
    ${definitions.join("\n")}
    return {pending,events,elements,start:doRun,kill:killRun,switchPage:confirmStopForPageSwitch,
      redraw:renderStep,holdCompose(){composeYaml=()=>defer('compose');},holdEdit(){projectEditQueue=defer('edit');},
      state(){return {busy:runInProgress,stopping:killInProgress,result:runInfo,completion:runCompletion};}};
  `);
  const flush = () => new Promise(resolve=>setImmediate(resolve));
  const take = (h,command) => { const i=h.pending.findIndex(p=>p.command===command);check(i>=0, `missing run command ${command}`);return h.pending.splice(i,1)[0]; };
  const done = outdir => ({ok:true,outdir,gridfile:outdir+'/mesh.nc',code:0});
  for (const outcome of ['resolve','reject']) {
    const h=harness(),first=h.start();await flush();const old=take(h,'run_project');
    check(h.events.some(e=>e[0]==='render' && !e[1] && e[2]===null), 'starting a rerun must repaint without old results');
    h.redraw();check(h.elements.runBtn.disabled && h.elements.killBtn.style.display==='inline-flex','redraw must preserve running controls');
    h.start();await flush();check(h.pending.length===0,'duplicate Run must not create a second command');
    let switched=false;const stop=h.switchPage().then(value=>{switched=value;});take(h,'kill_run').resolve(true);await flush();
    check(!switched && h.state().busy && h.elements.runBtn.disabled,'stop must wait for its run owner before allowing retry/page switch');
    h.start();await flush();check(h.pending.length===0,'retry must not overlap cancelled command settlement');
    old[outcome](outcome==='resolve'?{ok:false,code:null,outdir:'/cancelled'}:new Error('cancelled command'));
    await first;await stop;check(switched && !h.state().busy && !h.state().stopping,'settled cancellation must release navigation');
    const next=h.start();await flush();take(h,'run_project').resolve(done('/new'));await next;
    check(h.state().result.outdir==='/new' && h.events.filter(e=>e[0]==='load').map(e=>e[1]).join()==='/new/mesh.nc','recovery must load only its own successful mesh');
  }
  const editing=harness();editing.holdEdit();const waiting=editing.start();await flush();
  check(editing.state().busy && editing.pending.length===1,'Run must own startup but not compose before pending edits settle');
  take(editing,'edit').resolve();await flush();take(editing,'run_project').resolve(done('/after-edit'));await waiting;
  const early=harness();early.holdCompose();const composing=early.start();await flush();
  const noChild=early.switchPage();take(early,'kill_run').resolve(false);
  check(!await noChild && early.state().busy,'no child during startup is not a completed cancellation');
  take(early,'compose').reject(new Error('invalid project'));await composing;
  check(early.state().result?.ok===false && !early.state().busy && !early.state().completion,'compose failure must render failure and release its owner');

  const late=harness(),run=late.start();await flush();const child=take(late,'run_project');
  const stop=late.kill(),reply=take(late,'kill_run');child.resolve(done('/finished'));await run;
  late.redraw();check(late.elements.runBtn.disabled,'pending kill reply must keep retry fenced after natural completion');
  late.start();await flush();check(late.pending.length===0,'late stop must not race a newer child');
  reply.resolve(false);await stop;check(!late.elements.runBtn.disabled && !late.state().stopping,'late no-child reply must release retry without erasing success');
  check(late.state().result.outdir==='/finished','late stop reply must not replace completed result');
  const retry=late.start();await flush();take(late,'run_project').resolve(done('/retry'));await retry;
  check(late.state().result.outdir==='/retry','retry after late stop must succeed');
  log('run settlement: pending redraw, duplicate Run, stop/page-switch fence, late kill, compose failure and recovery passed');
}
checkRunSettlement().catch(error => { console.error(error); process.exitCode=1; });

// Exercise the real target callbacks and candidate commit, not a second UI implementation.
async function checkProjectEditAdmission() {
  const extract = name => section(html, new RegExp(`  ((?:async )?function ${name}\\([^\\n]*\\) \\{[\\s\\S]*?\\n  \\})`), name);
  const commitYaml = html.includes('function commitProjectYaml(') ? extract('commitProjectYaml') : '';
  const commit = html.includes('function commitProjectEdit(') ? extract('commitProjectEdit') : '';
  const harness = new Function(`
    let projectEditQueue=Promise.resolve(),baseProjectYaml=null,lastSummary=null,targetEdit=null,cellEdit=null;
    let colmMeshDelivery={enabled:false,pixelsPerDegree:240},rejectSummary=false,clears=0;
    const layerEdits={},logs=[],elements={targetKindOutput:{value:'atmosphere'},targetModelOutput:{value:'MPAS'},targetCellOutput:{value:'hex'}};
    const document={getElementById:id=>elements[id]},zh=()=>false,logLine=s=>logs.push(s);
    const compatibleTargetModels=()=>['MPAS','CoLM','FVCOM'],selectedTarget=()=>targetEdit||{kind:'atmosphere'};
    const wireExpertTargetStep=()=>{},clearRunArtifacts=()=>{clears++;};
    const initial={target_kind:'atmosphere',model_format:'MPAS',cell:'hex',layers:['a','b'].map(id=>({id,path:'',enabled:false,source_field:'landtype'}))};
    function validate(cfg){
      if(['land','ocean'].includes(cfg.target_kind)&&!cfg.layers.some(l=>l.enabled&&l.path))throw new Error('LandType required');
      if(cfg.layers.some(l=>l.enabled&&!l.path))throw new Error('empty source');
      return JSON.stringify(cfg);
    }
    const api={
      setProjectTarget:async(yaml,kind,model)=>validate({...JSON.parse(yaml),target_kind:kind,model_format:model,colm_mesh_enabled:false}),
      setTargetCell:async(yaml,cell)=>{if(!['tri','hex'].includes(cell))throw new Error('bad cell');return validate({...JSON.parse(yaml),cell});},
      summary:async yaml=>{if(rejectSummary)throw new Error('summary unavailable');validate(JSON.parse(yaml));return JSON.parse(yaml);},
      validate:async yaml=>validate(JSON.parse(yaml))
    };
    async function invoke(command,{yaml,id,path,enabled}){
      if(command!=='set_layer_path')throw new Error(command);
      const cfg=JSON.parse(yaml),selected=cfg.layers.find(l=>l.id===id);
      if(!selected)throw new Error('unknown source');
      if(enabled)cfg.layers.forEach(l=>{if(l.source_field===selected.source_field)l.enabled=false;});
      Object.assign(selected,{path,enabled});return validate(cfg);
    }
    async function composeYaml(){
      let yaml=baseProjectYaml||validate(initial);
      if(targetEdit)yaml=await api.setProjectTarget(yaml,targetEdit.kind,targetEdit.modelFormat);
      if(cellEdit)yaml=await api.setTargetCell(yaml,cellEdit);
      for(const id of Object.keys(layerEdits).sort((a,b)=>Number(layerEdits[b].enabled)-Number(layerEdits[a].enabled)))
        yaml=await invoke('set_layer_path',{yaml,id,...layerEdits[id]});
      return yaml;
    }
    function paintTargetOutputs(s){if(s){elements.targetKindOutput.value=s.target_kind;elements.targetModelOutput.value=s.model_format;elements.targetCellOutput.value=s.cell;}}
    ${extract('refreshSummary')}
    ${commitYaml}
    ${commit}
    ${extract('enhanceTargetOutputStep')}
    return {init:enhanceTargetOutputStep,logs,elements,composeYaml,
      change:async(id,value)=>{elements[id].value=value;await elements[id].onchange();},
      edit:fn=>commitProjectEdit(fn),source:(id,path,enabled)=>commitProjectEdit(yaml=>invoke('set_layer_path',{yaml,id,path,enabled})),
      failSummary:value=>{rejectSummary=value;},
      state:()=>JSON.stringify({baseProjectYaml,lastSummary,targetEdit,cellEdit,colmMeshDelivery,layerEdits,clears})};
  `);
  for(const [name,after] of [['onSave','composeYaml()'],['onOpen','api.openProject()'],['reflectProject','api.summary(res.yaml)'],['onNew','resetProject()'],['openRecent','invoke("read_project"']]) {
    const body=extract(name);
    check(body.indexOf('await projectEditQueue;')>=0&&body.indexOf('await projectEditQueue;')<body.indexOf(after),name+' must wait for pending edits before consuming/replacing project');
  }
  const template=section(html,/async function selectTemplate\(k\)\{([\s\S]*?)\n\}/,'template edit fence');
  check(template.indexOf('await window.waitForProjectEdits()')>=0&&template.indexOf('await window.waitForProjectEdits()')<template.indexOf('tpl=next;'),'template change must wait for pending edits');
  const h=harness();await h.init();
  const before=h.state();
  for(const kind of ['land','ocean']){
    await h.change('targetKindOutput',kind);
    check(h.state()===before && h.elements.targetKindOutput.value==='atmosphere','rejected target edit must keep the valid project, summary and target selection');
  }
  check(h.logs.some(s=>s.includes('LandType required')),'rejected edit must report backend reason');
  for(const kind of ['land','ocean']){
    check(await h.source('a','/land-a.nc',true),'adding source should succeed before target migration');
    await h.change('targetKindOutput',kind);
    check(JSON.parse(await h.composeYaml()).target_kind===kind,'accepted source must survive target-before-source composition');
    const withSource=h.state();
    check(!await h.source('a','',false)&&h.state()===withSource,'required-source clear must leave valid state unchanged');
    check(!await h.source('a','/land-a.nc',false)&&h.state()===withSource,'required-source disable must leave valid state unchanged');
    check(await h.source('b','/land-b.nc',true),'exclusive source replacement should succeed');
    const replaced=JSON.parse(await h.composeYaml());
    check(replaced.layers.filter(l=>l.enabled).map(l=>l.id).join()==='b','backend exclusivity must survive compose');
    await h.change('targetKindOutput','atmosphere');
    check(await h.source('b','',false),'source removal should succeed after leaving a required-source target');
    check(JSON.parse(await h.composeYaml()).layers.every(l=>!l.enabled),'removed sources must not revive from original base');
  }
  await h.change('targetModelOutput','CoLM');await h.change('targetCellOutput','tri');
  check(JSON.parse(await h.composeYaml()).cell==='tri'&&h.elements.targetModelOutput.value==='CoLM','model and cell handlers must commit validated candidates');
  const valid=h.state();h.failSummary(true);
  check(!await h.source('a','/candidate.nc',true)&&h.state()===valid,'summary failure must not partially commit candidate');
  h.failSummary(false);
  let release;const blocked=new Promise(resolve=>{release=resolve;});let secondStarted=false;
  const first=h.edit(async yaml=>{await blocked;return yaml;});
  const second=h.edit(async yaml=>{secondStarted=true;return yaml;});
  await new Promise(resolve=>setImmediate(resolve));check(!secondStarted,'project edits must serialize');
  release();check(await first&&await second&&secondStarted,'serialized edits must settle and remain usable after rejection');
  log('project edit admission: rejected target/source recovery, canonical migrations, exclusivity, model/cell and serialized commits passed');
}
checkProjectEditAdmission().catch(error => { console.error(error); process.exitCode=1; });

{
  const TPL = new Function('return '+section(html,/const TPL=(\[[\s\S]*?\n\]);/,'template cards'))();
  const gallery = section(html,/(<div class="tpl">[\s\S]*?)\n      <div class="grid2">/,'template gallery');
  const render = new Function('TPL','tpl','lang','return `'+gallery+'`;');
  for (const lang of [0,1]) for (const selected of [0,TPL.length-1]) {
    const markup = render(TPL,selected,lang);
    const buttons = [...markup.matchAll(/<button\b([^>]*)>([\s\S]*?)<\/button>/g)];
    check(buttons.length===TPL.length,'every template card must be a native keyboard-accessible button');
    buttons.forEach(([,attrs,body],k)=>{
      check(attrs.includes('type="button"')&&attrs.includes(`data-tpl="${k}"`)&&
        attrs.includes(`aria-pressed="${k===selected}"`)&&attrs.includes(`aria-label="${TPL[k].nm[lang]}"`),
        'template buttons must expose their name and committed selection, without submitting forms');
      check(!/<(?:div|button|input|select|a)\b/.test(body),'template button content must be noninteractive phrasing content');
    });
  }
  log('template accessibility: native buttons expose bilingual names and committed selection');
}

async function checkTemplateAdmission() {
  const extract=name=>{const indent=name==="selectTemplate"?"":"  ";return section(html,new RegExp(`${indent}((?:async )?function ${name}\\([^\\n]*\\) ?\\{[\\s\\S]*?\\n${indent}\\})`),name);};
  const reset=section(html,/window\.resetTemplateDerivedState = function \(\) \{([\s\S]*?)\n  \};/,'template reset');
  const bridge=html.match(/  window\.commitTemplateEdit = [\s\S]*?\n  \};/)?.[0]||'';
  const commitYaml=html.includes('function commitProjectYaml(')?extract('commitProjectYaml'):'';
  const preset=html.includes('function templateSpecifiedRefinement(')?extract('templateSpecifiedRefinement'):'';
  const harness=new Function(`
    const TPL=${section(html,/const TPL=(\[[\s\S]*?\n\]);/,'template cards')};
    let tpl=0,domainMode='global',regional=false,watershedPath='',closePath='',closeFormat='nml',hiddenDomainShape=null;
    let domainEdit=null,domainCloseBoundary={},resUnitIdx=1,resVal=3,maxPasses=3,targetEdit=null,cellEdit=null;
    let specifiedRefine={enabled:false,algorithm:'certified',route:'discrete'},colmMeshDelivery={enabled:false,pixelsPerDegree:240};
    let projectEditQueue=Promise.resolve(),backendReady=null,cur=1,clears=0,paints=0,focused=null;
    const layerEdits={},thresholdEdits={},criterionEdits={},metadataEdit={authors:['author'],description:'keep'};
    const qualityEdit={minAngle:31,policy:'warn',batchCells:1},expertEdit={},hydroRefine={},thresholdRefine={enabled:false};
    const DEFAULT_BBOX=[108,120,18,26],domBbox=[110,118,20,25],METHOD_C_MAX_REFINEMENT_LEVEL=5;
    let baseProjectYaml=JSON.stringify({intent:'AtmosphereMpas',target_kind:'atmosphere',cell:'hex',model_format:'MPAS',domain:'global',layers:[],hidden:'keep'});
    let lastSummary={...JSON.parse(baseProjectYaml),_valid:true,_err:null};
    const logs=[],calls=[],window={},zh=()=>false,logLine=s=>logs.push(s);
    const document={querySelector:selector=>({focus(){focused=selector;}})};
    const currentIntent=()=>TPL[tpl].intent,currentResolution=()=>({nxp:resVal,approxKm:null,approxDegree:null}),projectName=()=>'template-test';
    const normalizeCloseBoundary=()=>({mode:'polyline'}),defaultAlgorithmControls=()=>({}),inferCloseFormat=()=>'nml';
    const clearCoastalOverlay=()=>{},clearRunArtifacts=()=>{clears++;},renderSteps=()=>{},renderStep=()=>{paints++;};
    const springTypesFor=()=>({}),hasEnabledThresholdLayer=()=>false,hasEnabledHydroRefinement=()=>false;
    const applyCloseBoundary=async yaml=>yaml;
    function validate(cfg){if(['land','ocean'].includes(cfg.target_kind)&&!cfg.layers.some(l=>l.enabled&&l.path))throw new Error('landtype required');return JSON.stringify(cfg);}
    async function invoke(command,args){
      calls.push([command,args]);let cfg=args.yaml?JSON.parse(args.yaml):null;
      if(command==='scaffold_project')return validate({intent:args.intent,target_kind:args.intent==='AtmosphereMpas'?'atmosphere':args.intent==='CoastalOcean'?'ocean':'land',cell:args.intent==='CoastalOcean'?'tri':'hex',model_format:args.intent==='AtmosphereMpas'?'MPAS':args.intent==='CoastalOcean'?'FVCOM':'CoLM',domain:'global',nxp:args.nxp,layers:[{id:'landcover',enabled:true,path:'/preset.nc'}]});
      if(command==='preserve_unexposed_project_fields'){
        const base=JSON.parse(args.baseYaml);cfg.layers=base.layers.length?base.layers:[{id:'landcover',enabled:false,path:''}];cfg.hidden=base.hidden;
        if(base.intent===cfg.intent)for(const k of ['target_kind','cell','model_format'])cfg[k]=base[k];
      }else if(command==='set_project_target'){cfg.target_kind=args.kind;cfg.model_format=args.modelFormat;}
      else if(command==='set_target_cell')cfg.cell=args.cell;
      else if(command==='set_domain_global')cfg.domain='global';
      else if(command==='set_domain_bbox'){cfg.domain='regional';cfg.bbox=[args.w,args.e,args.s,args.n];}
      else if(command==='set_domain_close'){cfg.domain='regional';cfg.close=args.path;}
      else if(command==='set_layer_path'){const l=cfg.layers.find(l=>l.id===args.id);if(!l)throw new Error('missing source');Object.assign(l,{path:args.path,enabled:args.enabled});}
      else if(command==='set_specified_refinement')cfg.specified=args.enabled?{kind:args.kind,path:args.path}:null;
      else if(command==='set_refinement_backend')cfg.backend=args.backend;
      else if(command==='set_refinement')cfg.max_passes=args.maxPasses;
      else if(command==='set_quality')cfg.min_angle_deg=args.minAngleDeg;
      else if(command==='set_project_metadata')cfg.description=args.description;
      validate(cfg);return command==='project_summary'?cfg:JSON.stringify(cfg);
    }
    const api={summary:yaml=>invoke('project_summary',{yaml})};
    ${preset}
    ${extract('composeYaml')}
    ${commitYaml}
    ${extract('commitProjectEdit')}
    window.waitForProjectEdits=()=>projectEditQueue;
    window.resetTemplateDerivedState=function(){${reset}};
    ${bridge}
    ${extract('selectTemplate')}
    return {choose:selectTemplate,compose:composeYaml,logs,calls,
      source(){const cfg=JSON.parse(baseProjectYaml);cfg.layers=[{id:'landcover',enabled:true,path:'/chosen.nc'}];baseProjectYaml=validate(cfg);lastSummary={...cfg,_valid:true,_err:null};},
      customize(){const cfg=JSON.parse(baseProjectYaml);Object.assign(cfg,{target_kind:'atmosphere',model_format:'ICON',cell:'tri'});baseProjectYaml=validate(cfg);targetEdit={kind:'atmosphere',modelFormat:'ICON'};cellEdit='tri';},
      hold(){let release;projectEditQueue=new Promise(resolve=>{release=resolve;});return release;},
      state:()=>JSON.stringify({tpl,domainMode,regional,watershedPath,closePath,closeFormat,hiddenDomainShape,domainEdit,domainCloseBoundary,resUnitIdx,resVal,maxPasses,targetEdit,cellEdit,specifiedRefine,baseProjectYaml,lastSummary,layerEdits,clears,paints,focused})};
  `);
  const empty=harness(),before=empty.state();
  for(const card of [1,2,7,8,9]){
    await empty.choose(card);
    check(empty.state()===before,'rejected template must preserve original domain, source/target state, resolution, refinement and results');
    check(JSON.parse(await empty.compose()).target_kind==='atmosphere','rejected template must leave project composable');
  }
  check(empty.logs.some(s=>s.includes('landtype required')),'template rejection must expose backend reason');
  const h=harness();h.source();await h.choose(1);
  check(JSON.parse(h.state()).focused==='[data-tpl="1"]','accepted template must restore focus after replacing its button');
  await h.choose(1);check(JSON.parse(h.state()).focused==='[data-tpl="1"]','reselecting the active template must retain keyboard focus');
  let cfg=JSON.parse(await h.compose());check(cfg.target_kind==='land'&&cfg.layers[0].path==='/chosen.nc','different-intent template must use preset target and preserve chosen source');
  h.customize();await h.choose(4);cfg=JSON.parse(await h.compose());
  check(cfg.domain==='regional'&&cfg.target_kind==='atmosphere'&&cfg.model_format==='ICON'&&cfg.cell==='tri','same-intent regional preset must retain canonical target overrides');
  for(const [card,nxp] of [[7,80],[8,768],[9,192]]){
    await h.choose(card);cfg=JSON.parse(await h.compose());
    check(cfg.nxp===nxp&&cfg.close==='input/Ocean/Ocean_ChinaSea_boundary.nml','close templates must apply their own resolution and domain only after admission');
    check(cfg.layers[0].path==='/chosen.nc'&&cfg.hidden==='keep'&&cfg.min_angle_deg===31&&cfg.description==='keep','templates must preserve common source/hidden/quality/metadata fields');
    if(card===9)check(cfg.max_passes===2&&cfg.specified?.path==='input/Ocean/refine_spc_close01.nml','O3 specified-close and passes must survive next compose');
  }
  const blocked=harness(),release=blocked.hold(),initial=blocked.state(),pending=blocked.choose(3);
  await new Promise(resolve=>setImmediate(resolve));check(blocked.state()===initial,'template selection must not mutate while earlier edit is pending');
  release();await pending;check(JSON.parse(await blocked.compose()).domain==='regional','template must apply after earlier edit settles');
  log('template admission: actual selector/compose rejects atomically, preserves common and same-intent state, applies O1/O2/O3 and waits for edits');
}
checkTemplateAdmission().catch(error=>{console.error(error);process.exitCode=1;});

async function checkWorkflowNavigation() {
  const extract=name=>section(html,new RegExp(`((?:async )?function ${name}\\([^\\n]*\\)\\{[\\s\\S]*?\\n\\})`),name);
  const steps=section(html,/const STEPS=(\[[\s\S]*?\n\]);/,'workflow steps');
  const harness=new Function('lang',`
    const STEPS=${steps};let cur=0,runInProgress=false,killInProgress=false,answer=true,stop=null,focused=null;
    const paints=[],prompts=[];
    const element=tag=>({tag,attrs:{},children:[],setAttribute(k,v){this.attrs[k]=v;},
      set textContent(v){this.text=v;this.children=[];},append(...nodes){this.children.push(...nodes);},
      appendChild(node){this.append(node);},focus(){focused=this;}});
    const rail=element('nav'),heading=element('h1'),work={scrollTop:100,querySelector:s=>s==='h1'?heading:null};
    const document={createElement:element,getElementById:id=>id==='steps'?rail:work};
    const confirm=message=>{prompts.push(message);return answer;};
    const killRun=()=>new Promise(resolve=>{stop=ok=>{if(ok){runInProgress=false;killInProgress=false;}resolve(ok);};});
    const renderStep=i=>paints.push(i);
    ${extract('renderSteps')}
    ${extract('confirmStopForPageSwitch')}
    ${extract('go')}
    renderSteps();return {STEPS,rail,heading,work,paints,prompts,go,
      click(i){const b=rail.children[i];b.focus();return b.onclick();},
      guard(running,accept,killing=false){runInProgress=running;answer=accept;killInProgress=killing;},
      stop(ok){stop(ok);stop=null;},state:()=>({cur,focused,pending:!!stop})};
  `);
  for(const lang of [0,1]) {
    const h=harness(lang);
    check(h.rail.children.length===7,'workflow rail must expose every step');
    h.rail.children.forEach((button,i)=>{
      check(button.tag==='button'&&button.type==='button','workflow steps must be native non-submit buttons');
      check(button.attrs['aria-current']===(i===0?'step':undefined),'workflow current state must match committed page');
      const [number,copy]=button.children;
      check(number.tag==='span'&&copy.tag==='span'&&copy.children.every(n=>n.tag==='span'),'workflow buttons must contain phrasing elements');
      check(copy.children[0].text===h.STEPS[i].t[lang]&&copy.children[1].text===h.STEPS[i].d[lang],'workflow labels must retain bilingual safe text');
    });
    await h.click(3);
    check(h.state().cur===3&&h.state().focused===h.heading&&h.work.scrollTop===0,'accepted navigation must focus the destination heading and reset scroll');
    check(h.rail.children[3].attrs['aria-current']==='step'&&!h.rail.children[0].attrs['aria-current'],'only the committed step may be current');
    const before=h.paints.length,oldButtons=h.rail.children;
    h.guard(true,false);await h.click(1);
    check(h.state().cur===3&&h.paints.length===before&&h.rail.children===oldButtons&&h.state().focused===oldButtons[1],'declined stop must preserve page and triggering focus');
    for(const ok of [false,true]) {
      h.guard(!ok,true,ok);const pending=h.click(1);await new Promise(resolve=>setImmediate(resolve));
      check(h.state().pending&&h.state().cur===3&&h.paints.length===before,'navigation must await running or settling kill guard');
      h.stop(ok);await pending;
      check(h.state().cur===(ok?1:3)&&h.state().focused===(ok?h.heading:oldButtons[1]),'only successful stop may navigate and move focus');
    }
    const prompts=h.prompts.length;await h.click(1);check(h.prompts.length===prompts,'same-step activation must not request a stop');
    check(await h.go(6)&&h.state().focused===h.heading,'footer shared go must use the same destination focus');
  }
  check(html.includes('<h1 tabindex="-1">${t}</h1>'),'workflow heading must support programmatic focus without adding a tab stop');
  check(html.includes('b.onclick=()=>go(+b.dataset.go)'),'footer and rail must share the guarded go handler');
  log('workflow accessibility: native bilingual steps, committed current state, destination focus and stop/page-switch guard passed');
}
checkWorkflowNavigation().catch(error=>{console.error(error);process.exitCode=1;});

async function checkProjectControls() {
  const extract=name=>section(html,new RegExp(`  ((?:async )?function ${name}\\([^\\n]*\\) \\{[\\s\\S]*?\\n  \\})`),name);
  const picker=section(html,/(<(?:div|button)[^>\n]*id="outPathBrowse"[\s\S]*?<\/(?:div|button)>)/,'output folder picker');
  const renderPicker=new Function('lang','return `'+picker+'`;');
  for(const lang of [0,1]) {
    const markup=renderPicker(lang);
    check(markup.startsWith('<button type="button"')&&markup.includes(`aria-label="${lang?'选择输出目录':'Choose output folder'}"`)&&markup.includes('aria-describedby="outPathText"'),'output picker must be a named native button describing the current path');
  }
  const harness=new Function('chinese',`
    let outputPath='/before',projectEditQueue=Promise.resolve(),project='before',pick=null,readFails=false;
    let recents=[{path:'/a/<project>.yaml',name:'<img src=x onerror=bad>'}];
    const logs=[],reads=[],saved=[],metadataEdit={},zh=()=>chinese,logLine=s=>logs.push(s);
    const element=tag=>({tag,style:{},children:[],textContent:'',appendChild(n){n.parent=this;this.children.push(n);},remove(){this.parent.children=this.parent.children.filter(n=>n!==this);}});
    const card=element('div'),hint=element('div'),folder=element('button'),pathText=element('span');
    card.querySelector=s=>s==='h3'?{textContent:chinese?'最近项目':'Recent projects'}:hint;
    card.querySelectorAll=()=>card.children.slice();
    const document={createElement:element,getElementById:id=>({outPathBrowse:folder,outPathText:pathText})[id]||null,
      querySelector:()=>null,querySelectorAll:()=>[card]};
    const loadRecents=()=>recents,renderProjectSummary=()=>{},localStorage={setItem:(...args)=>saved.push(args)};
    const api={pickDataFolder:async()=>{if(pick instanceof Error)throw pick;return pick;}};
    const invoke=async(command,args)=>{reads.push([command,args]);if(readFails)throw new Error('missing project');return {yaml:'opened',path:args.path};};
    const reflectProject=async res=>{project=res.yaml;};
    ${extract('openRecent')}
    ${extract('enhanceNewProjectStep')}
    enhanceNewProjectStep();return {card,folder,pathText,logs,reads,saved,render:enhanceNewProjectStep,
      pick(p){pick=p;return folder.onclick();},readFail(v){readFails=v;},empty(){recents=[];enhanceNewProjectStep();},
      hold(){let release;projectEditQueue=new Promise(resolve=>{release=resolve;});return release;},state:()=>({outputPath,project})};
  `);
  for(const chinese of [false,true]) {
    const h=harness(chinese),row=h.card.children[0];
    check(row.tag==='button'&&row.type==='button','recent projects must be native non-submit buttons');
    check(row.title==='/a/<project>.yaml'&&row.children[0].textContent==='📄 <img src=x onerror=bad>','recent names and paths must stay literal text');
    h.render();check(h.card.children.length===1,'project page enhancement must not duplicate recent buttons');
    const before=JSON.stringify(h.state());
    await h.pick(null);await h.pick(new Error('picker failed'));
    check(JSON.stringify(h.state())===before&&h.saved.length===0&&h.logs.some(s=>s.includes('picker failed')),'cancelled or failed picker must preserve output preference and report failure');
    await h.pick('/selected/<folder>');
    check(h.state().outputPath==='/selected/<folder>'&&h.pathText.textContent==='📁 /selected/<folder>'&&h.saved[0].join()==='em.outputPath,/selected/<folder>','accepted picker must update safe visible text and existing saved preference');
    h.readFail(true);await h.card.children[0].onclick();
    check(h.state().project==='before'&&h.logs.some(s=>s.includes('missing project')),'failed recent read must leave the current project unchanged');
    h.readFail(false);const release=h.hold(),reads=h.reads.length,pending=h.card.children[0].onclick();
    await new Promise(resolve=>setImmediate(resolve));check(h.reads.length===reads,'recent activation must wait for pending edits');
    release();await pending;check(h.state().project==='opened'&&h.reads.at(-1)[1].path==='/a/<project>.yaml','accepted recent button must use the original shared read path');
    h.empty();check(h.card.children.length===1&&h.card.children[0].className==='recent-empty','empty recents must keep the noninteractive placeholder');
  }
  const reflect=extract('reflectProject');
  check(reflect.indexOf('document.querySelector("#work h1")?.focus();')>reflect.indexOf('renderStep(typeof cur'),'shared Open/recent reflection must restore heading focus after rerender');
  log('project controls: native recent/folder buttons, safe text, picker cancel/failure, recent read failure and edit fence passed');
}
checkProjectControls().catch(error=>{console.error(error);process.exitCode=1;});

// Exercise the real circle validation/frame/estimate and domain compose branch
// without a browser dependency, so this boundary also runs in the regular CI gate.
async function checkCircleDomain() {
  const extract=name=>section(html,new RegExp(`(function ${name}\\([^\\n]*\\)\\{[\\s\\S]*?\\n\\})`),name);
  const compose=section(html,/async function composeYaml\([^)]*\) \{([\s\S]*?)\n  \}/,'composeYaml');
  const domain=compose.slice(compose.indexOf('    const mode ='),compose.indexOf('    if (baseProjectYaml)'));
  const h=new Function(`
    let domCircle=[179,20,750],domainMode='circle',regional=true,lang=0;
    const KM_PER_DEG_EQ=2*Math.PI*6371.229/360,DEFAULT_BBOX=[108,120,18,26],domBbox=DEFAULT_BBOX;
    const cellKm=()=>100,olGeojsonFrame=()=>{throw Error('circle fell back to an old mesh frame');};
    ${extract('wrapOlLon')}
    ${extract('circleDomainError')}
    ${extract('currentOlDomainFrame')}
    ${extract('estCells')}
    return {set:c=>{domCircle=c;},error:circleDomainError,frame:currentOlDomainFrame,estimate:estCells,
      compose:async()=>{const template=null,domain={kind:'circle',seaRatio:.47125},calls=[];let yaml='input';
        const invoke=async(cmd,args)=>{calls.push({cmd,args});return 'circle yaml';};
        ${domain}
        return {yaml,calls};}};
  `)();
  check(h.frame().crossesDateline && h.frame().east-h.frame().west<20,'circle frame must use the short dateline span');
  const {yaml,calls}=await h.compose();
  check(yaml==='circle yaml'&&calls.length===1&&calls[0].cmd==='set_domain_circle','circle must reach its shared setter, not bbox');
  check(JSON.stringify(calls[0].args)===JSON.stringify({yaml:'input',lon:179,lat:20,radiusKm:750,seaRatio:.47125}),'circle coordinates/radius/sea ratio must retain precision');
  const radius=6371.229,area=4*Math.PI*radius**2*Math.sin(750/radius/2)**2;
  check(h.estimate()===Math.round(area/(100*100*.866)),'circle estimate must use spherical-cap area');
  for(const circle of [[NaN,20,750],[181,20,750],[0,91,750],[0,0,0],[0,0,10009]]){
    h.set(circle);check(h.error()&&h.frame()===null&&h.estimate()===0,'invalid circle must not show an old mesh extent/estimate');
    let rejected=false;try{await h.compose();}catch{rejected=true;}check(rejected,'invalid draft must fail compose before any setter');
  }
  for(const lat of [-90,90]){h.set([0,lat,500]);const frame=h.frame();check(frame.west===-180&&frame.east===180,'polar circles cover all longitudes');}
  check(libRs.includes('set_domain_circle,')&&html.includes('domCircle = [...sum.circle]')&&html.includes('circle:domCircle')&&html.includes('if ("circle" in payload) domCircle = payload.circle;'),'circle command, open reflection and detached map state must remain wired');
  check(html.includes('data-mode="circle"')&&html.includes('aria-describedby="domainCircleHint domainCircleError"'),'circle editor needs a named, described native input');
  log('circle domain: actual compose/validation/dateline/pole/area and command/reflection/map state checks passed');
}
checkCircleDomain().catch(error=>{console.error(error);process.exitCode=1;});
