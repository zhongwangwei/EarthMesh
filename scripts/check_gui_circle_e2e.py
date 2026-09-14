#!/usr/bin/env python3
"""Domain editor/round-trip/map regression, using real GUI summary fixtures.

python3 scripts/check_gui_circle_e2e.py /path/to/gui-records.json
Transport is mocked; Rust GUI tests separately exercise validation and real CLI delivery.
Uses the existing Playwright installation; installs nothing.
"""
import json
import math
import sys
import threading
from http.server import ThreadingHTTPServer

from check_gui_map_e2e import QuietHandler
from pathlib import Path

from playwright.sync_api import sync_playwright


def main():
    source = Path(sys.argv[1]).resolve()
    records = json.loads(source.read_text())
    summary = next(c["summary"] for c in records["cases"] if c["summary"]["domain_shape"] == "circle")
    assert summary["domain_shape"] == "circle" and len(summary["circle"]) == 3
    summary = {**summary, "sea_ratio": 0.47125, "unexposed_probe": "keep"}
    errors = []
    root = Path(__file__).resolve().parents[1]
    with ThreadingHTTPServer(("127.0.0.1", 0), lambda *a, **k: QuietHandler(*a, directory=str(root), **k)) as server, sync_playwright() as p:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1400, "height": 900})
        page.on("pageerror", lambda e: errors.append(str(e)))
        page.route("https://**", lambda r: r.abort())
        init = "window.__caps=" + json.dumps(records["capabilities"]) + ";window.__opened=" + json.dumps(summary) + ";"
        init += """
          window.__calls=[];window.__events=[];window.__listeners={};
          window.__TAURI__={webviewWindow:{WebviewWindow:{}},event:{
            listen:async(name,fn)=>{window.__listeners[name]=fn;return ()=>{};},
            emitTo:async(target,event,payload)=>window.__events.push({target,event,payload})
          },core:{invoke:async(command,args={})=>{
            window.__calls.push({command,args});
            if(command==='project_capabilities') return window.__caps;
            if(command==='list_criteria') return [];
            if(command==='open_project') return {path:'/circle.yaml',yaml:JSON.stringify(window.__opened)};
            if(command==='save_project') {window.__saved=JSON.parse(args.yaml);return '/saved-circle.yaml';}
            if(command==='run_project') {window.__run=JSON.parse(args.yaml);return {ok:false,code:2,outdir:'/mock-run',gridfile:null,delivery:null};}
            if(command==='validate_project') return [];
            if(command==='pick_data_file') {if(window.__holdPicker)return new Promise((resolve,reject)=>window.__picker={resolve,reject});if(window.__pickError)throw Error('picker failed');return window.__pick??null;}
            if(command==='shapefile_boundary_geojson') {if(window.__holdBoundary)return new Promise((resolve,reject)=>window.__boundaries.push({path:args.path,resolve,reject}));return {type:'FeatureCollection',features:[]};}
            if(command==='scaffold_project') return JSON.stringify(window.__opened);
            const cfg=args.yaml?JSON.parse(args.yaml):null;
            if(command==='project_summary') {const summary={...cfg};if(window.__legacySummary)delete summary.circle;return summary;}
            if(command==='preserve_unexposed_project_fields' && args.preserveDomain){
              const old=JSON.parse(args.baseYaml);for(const key of ['domain','domain_shape','circle','bbox','sea_ratio'])cfg[key]=old[key];
            }
            if(command==='set_specified_refinement'){
              for(const key of ['lon','lat','radiusKm','w','e','s','n'])if(args[key]!=null && (typeof args[key]!=='number' || !Number.isFinite(args[key])))throw Error('invalid numeric argument '+key);
              cfg.specified_refine_enabled=args.enabled;cfg.specified_refine_kind=args.kind;
              if(args.kind==='radius')Object.assign(cfg,{specified_refine_lon:args.lon??0,specified_refine_lat:args.lat??0,specified_refine_radius_km:args.radiusKm??100});
              if(args.kind==='bbox')cfg.specified_refine_bbox=[args.w??0,args.e??1,args.s??0,args.n??1];
              if(args.kind==='close')cfg.specified_refine_path=args.path;
            }
            if(command==='set_domain_circle')Object.assign(cfg,{domain:'regional',domain_shape:'circle',circle:[args.lon,args.lat,args.radiusKm],bbox:null,sea_ratio:args.seaRatio});
            if(command==='set_domain_bbox')Object.assign(cfg,{domain:'regional',domain_shape:'bbox',circle:null,bbox:[args.w,args.e,args.s,args.n],sea_ratio:args.seaRatio});
            if(command==='set_domain_shapefile')Object.assign(cfg,{domain:'regional',domain_shape:'shapefile',circle:null,bbox:null,watershed_path:args.path,sea_ratio:args.seaRatio});
            if(command==='set_domain_close')Object.assign(cfg,{domain:'regional',domain_shape:'close',circle:null,bbox:null,watershed_path:args.path,close_format:args.format,sea_ratio:args.seaRatio});
            if(command==='set_domain_global')Object.assign(cfg,{domain:'global',domain_shape:'global',circle:null,bbox:null,sea_ratio:null});
            return cfg?JSON.stringify(cfg):null;
          }}};
        """
        page.add_init_script(init)
        url = f"http://127.0.0.1:{server.server_port}/gui-tauri/dist/index.html"
        page.goto(url)
        page.wait_for_function("document.getElementById('logStatus').textContent.includes('Rust')")

        def step(n):
            page.evaluate("n=>{cur=n;renderStep(n);renderSteps();}", n)

        def open_project(mode="circle"):
            step(0)
            page.locator("#projOpen").click()
            page.wait_for_function("mode=>domainMode===mode && !hiddenDomainShape", arg=mode)
            step(2)

        def save():
            step(0)
            page.evaluate("window.__saved=null")
            page.locator("#projSave").click()
            page.wait_for_function("!!window.__saved")
            return page.evaluate("window.__saved")

        # Specified geometry is a required draft, not optional threshold/default input.
        opened = {**summary, "specified_refine_enabled": True, "specified_refine_kind": "radius",
                  "specified_refine_lon": 113.125, "specified_refine_lat": 22.5,
                  "specified_refine_radius_km": 150, "specified_refine_circle_count": 1}
        page.evaluate("c=>window.__opened=c", opened)
        open_project()
        step(4)
        page.locator("#specifiedLon").wait_for()
        page.locator("#specifiedRefinementPanel").screenshot(path=str(source.parent / "specified-before-or-after.png"))
        for kind, fields in (("radius", (("specifiedLon", "113.125"), ("specifiedLat", "22.5"), ("specifiedRadius", "150"))),
                             ("bbox", (("specifiedW", "170"), ("specifiedE", "-170"), ("specifiedS", "-10"), ("specifiedN", "10")))):
            page.select_option("#specifiedKind", kind)
            for control, value in fields:
                page.locator(f"#{control}").fill(value)
            for control, value in fields:
                page.locator(f"#{control}").fill("")
                step(0)
                page.evaluate("window.__saved=null")
                page.locator("#projSave").click()
                page.wait_for_timeout(150)
                assert page.evaluate("window.__saved===null"), f"blank {control} silently saved a default"
                step(4)
                page.locator(f"#{control}").wait_for()
                assert page.locator(f"#{control}").input_value() == "", f"lost invalid {control} draft on navigation"
                page.evaluate("lang=1;applyI18n()")
                page.locator(f"#{control}").wait_for()
                assert page.locator(f"#{control}").input_value() == ""
                assert page.locator("#specifiedRefinementError").inner_text(), f"missing {control} error"
                assert page.locator(f"#{control}").get_attribute("aria-invalid") == "true"
                if control in ("specifiedLon", "specifiedW"):
                    # Inactive invalid geometry must not leak a string into Tauri Option<f64>.
                    page.locator("#specifiedRefineOn").uncheck()
                    assert not save()["specified_refine_enabled"]
                    step(4)
                    page.locator("#specifiedRefineOn").check()
                    page.locator(f"#{control}").wait_for()
                    assert page.locator(f"#{control}").input_value() == ""
                    page.select_option("#specifiedKind", "bbox" if kind == "radius" else "radius")
                    assert save()["specified_refine_enabled"]
                    step(4)
                    page.select_option("#specifiedKind", kind)
                    assert page.locator(f"#{control}").input_value() == ""
                    step(6)
                    page.evaluate("window.__run=null")
                    page.locator("#runBtn").click()
                    page.wait_for_function("!runInProgress")
                    assert page.evaluate("window.__run===null"), "invalid specified geometry reached run_project"
                    step(4)
                    page.locator(f"#{control}").wait_for()
                page.locator("#specifiedRefinementPanel").screenshot(path=str(source.parent / f"{kind}-invalid-zh.png"))
                page.locator(f"#{control}").fill(value)
                page.evaluate("lang=0;applyI18n()")
                page.locator(f"#{control}").wait_for()
            assert all(page.locator(f"#{control}").evaluate("el=>el.checkValidity()") for control, _ in fields)
            saved = save()
            if kind == "radius":
                assert [saved[k] for k in ("specified_refine_lon", "specified_refine_lat", "specified_refine_radius_km")] == [113.125, 22.5, 150]
            else:
                assert saved["specified_refine_bbox"] == [170, -170, -10, 10]
            step(4)
            page.locator("#specifiedKind").wait_for()
        # Refinement circles must NOT inherit the domain-only hemisphere limit.
        page.select_option("#specifiedKind", "radius")
        page.locator("#specifiedRadius").fill("0.0001")
        assert page.locator("#specifiedRadius").evaluate("el=>el.checkValidity()"), "native step/min rejects backend-valid radius"
        assert save()["specified_refine_radius_km"] == 0.0001
        step(4)
        page.locator("#specifiedRadius").fill("2e4")
        assert save()["specified_refine_radius_km"] == 20000
        step(4)
        page.select_option("#specifiedKind", "close")
        assert page.locator("#specifiedCloseBrowse").evaluate("el=>el.tagName") == "BUTTON"
        page.evaluate("window.__holdPicker=true")
        page.locator("#specifiedCloseBrowse").click()
        page.wait_for_function("!!window.__picker")
        step(0)
        page.evaluate("window.__picker.resolve('/obsolete.nml');window.__holdPicker=false")
        step(4)
        assert '/obsolete.nml' not in page.locator("#specifiedClosePathText").inner_text()
        page.evaluate("window.__pick='/accepted.nml'")
        page.locator("#specifiedCloseBrowse").focus()
        page.keyboard.press("Enter")
        page.wait_for_function("document.getElementById('specifiedClosePathText').textContent.includes('/accepted.nml')")
        assert save()["specified_refine_path"] == "/accepted.nml"
        # The existing backend preserves the full chain; this single-head editor is read-only.
        page.evaluate("c=>window.__opened=c", {**opened, "specified_refine_circle_count": 3})
        open_project()
        step(4)
        for language, width in ((1, 1000), (0, 1400)):
            page.set_viewport_size({"width": width, "height": 900})
            page.evaluate("l=>{lang=l;applyI18n();}", language)
            for control in ("specifiedLon", "specifiedLat", "specifiedRadius"):
                assert page.locator(f"#{control}").evaluate("el=>el.readOnly")
            panel = page.locator("#specifiedRefinementPanel")
            assert panel.evaluate("el=>el.scrollWidth<=el.clientWidth+1"), "specified controls overflow the card"
            panel.screenshot(path=str(source.parent / f"specified-chain-{width}.png"))
        assert save()["specified_refine_circle_count"] == 3
        page.evaluate("c=>window.__opened=c", summary)

        # Old domain forms must have the same draft/precision guarantees as Circle.
        original = summary.copy()
        bbox = [170, -170, -10, 10]
        for shape, mode, extra in (("bbox", "regional", {"bbox": bbox}), ("shapefile", "watershed", {"watershed_path": "/valid.shp"}), ("close", "close", {"watershed_path": "/valid.nml", "close_format": "nml"})):
            opened = {**original, "domain_shape": shape, "circle": None, "bbox": None, **extra}
            page.evaluate("c=>{window.__opened=c;domainMode='global';}", opened)
            open_project(mode)
            page.screenshot(path=str(source.parent / f"{shape}-before-or-after.png"))
            for language in (1, 0):
                page.evaluate("l=>{lang=l;applyI18n();}", language)
                assert float(page.locator("#seaRatioInput").input_value()) == 47.125, f"{shape} rounded sea ratio on render"
                assert save()["sea_ratio"] == 0.47125, f"{shape} changed sea ratio on save"
                step(2)
            for other in ("global", "circle", "regional"):
                page.locator(f'[data-mode="{other}"]').click()
                page.locator(f'[data-mode="{mode}"]').click()
                assert float(page.locator("#seaRatioInput").input_value()) == 47.125, f"{shape} lost sea ratio across mode switch"
            if shape == "bbox":
                fields = page.locator("#work .grid2 input.input")
                assert [float(v) for v in fields.evaluate_all("es=>es.map(e=>e.value)")] == bbox
                assert page.evaluate("estCells()") <= 2, "20 degree dateline bbox estimated as 340 degrees"
                fields.nth(0).fill("1e1")
                assert page.evaluate("domBbox[0]") == 10, "scientific notation must not become 11"
                for index, value in ((0, ""), (0, "181"), (2, "91"), (1, "10"), (3, "-10")):
                    fields.nth(index).fill(value)
                    page.evaluate("window.__calls=[];document.getElementById('logbox').textContent=''")
                    step(0)
                    page.locator("#projSave").click()
                    page.wait_for_function("document.getElementById('logbox').textContent.includes('save failed')")
                    assert not page.evaluate("window.__calls.some(c=>c.command==='save_project')")
                    step(2)
                    assert fields.nth(index).input_value() == value, "invalid bbox draft lost on navigation"
                    assert page.evaluate("currentOlDomainFrame()") is None
                    fields.nth(index).fill(str([10, -170, -10, 10][index]))
                fields.nth(0).fill("170")
            if shape in ("shapefile", "close"):
                id = "watershed" if shape == "shapefile" else "close"
                button = page.locator(f"#{id}Browse")
                assert button.evaluate("e=>e.tagName==='BUTTON'")
                page.evaluate("window.__pickError=true")
                button.click()
                page.wait_for_function("document.getElementById('logbox').textContent.includes('picker failed')")
                page.evaluate("window.__pickError=false")
                assert save()["watershed_path"] == opened["watershed_path"]
                step(2)
                for bad in ("/invalid.pdf", "/invalid.nc" if shape == "shapefile" else "/invalid.bin"):
                    page.evaluate("p=>window.__pick=p", bad)
                    button.click()
                    assert save()["watershed_path"] == opened["watershed_path"]
                    step(2)
                # A delayed native-picker result belongs to the old control, not the new mode.
                page.evaluate("window.__holdPicker=true;window.__picker=null")
                button.click()
                page.wait_for_function("!!window.__picker")
                page.locator('[data-mode="global"]').click()
                page.evaluate("async()=>{window.__picker.resolve('/late.shp');window.__holdPicker=false;await Promise.resolve();}")
                assert save()["domain_shape"] == "global"
                step(2)
                page.locator(f'[data-mode="{mode}"]').click()
                assert save()["watershed_path"] == opened["watershed_path"]
                step(2)
            if shape == "shapefile":
                page.evaluate("window.__holdBoundary=true;window.__boundaries=[];window.__pick='/pending.shp'")
                page.locator("#watershedBrowse").click()
                page.wait_for_function("window.__boundaries.length>0")
                page.locator('[data-mode="global"]').click()
                page.evaluate("async()=>{for(const b of window.__boundaries)b.resolve({type:'FeatureCollection',features:[{type:'Feature',properties:{old:true},geometry:{type:'Point',coordinates:[113,22]}}]});window.__holdBoundary=false;await Promise.resolve();}")
                assert page.evaluate("_domainGeojson===null"), "old shapefile preview repainted after global switch"
                page.locator(f'[data-mode="{mode}"]').click()
            if shape == "close":
                assert save()["close_format"] == "nml", "explicit close format replaced by filename inference"
                step(2)
                before = page.locator("#closePathText").inner_text()
                for pick in (None, "/invalid.pdf"):
                    page.evaluate("p=>window.__pick=p", pick)
                    page.locator("#closeBrowse").click()
                    assert page.locator("#closePathText").inner_text() == before
                    assert save()["watershed_path"] == opened["watershed_path"]
                    step(2)
            page.evaluate("hasRun=true;runInfo={ok:true,outdir:'/old'}")
            page.locator("#seaRatioInput").evaluate("e=>{e.value='32.125';e.dispatchEvent(new Event('input',{bubbles:true}));}")
            assert page.evaluate("!hasRun && runInfo===null"), f"{shape} ratio edit retained old success"
            assert save()["sea_ratio"] == 0.32125
        page.evaluate("c=>window.__opened=c", original)
        open_project()
        assert page.evaluate("JSON.stringify(domBbox)===JSON.stringify(DEFAULT_BBOX) && watershedPath==='' && closePath===''")
        assert [float(page.locator(f"#domainCircle{j}").input_value()) for j in range(3)] == summary["circle"]
        assert float(page.locator("#seaRatioInput").input_value()) == 47.125
        assert page.locator('[data-mode="circle"]').get_attribute("aria-pressed") == "true"
        # Native mode buttons must remain keyboard-operable and retain focus.
        for mode in ("global", "regional", "circle"):
            page.locator(f'[data-mode="{mode}"]').focus()
            page.keyboard.press("Enter")
            assert page.evaluate("document.activeElement.dataset.mode") == mode
            assert page.locator(f'[data-mode="{mode}"]').get_attribute("aria-pressed") == "true"
        assert float(page.locator("#seaRatioInput").input_value()) == 47.125, "circle sea ratio lost after mode switch"
        assert save()["sea_ratio"] == 0.47125
        open_project()
        # Invalid drafts survive navigation/language and cannot silently save earlier values.
        for index, value in ((0, ""), (0, "181"), (1, "91"), (2, "0"), (2, "10009")):
            page.locator(f"#domainCircle{index}").fill(value)
            assert page.locator("#domainCircleError").inner_text()
            assert page.locator(f"#domainCircle{index}").get_attribute("aria-invalid") == "true"
            assert page.evaluate("currentOlDomainFrame()") is None
            step(0)
            page.evaluate("window.__calls=[];document.getElementById('logbox').textContent=''")
            page.locator("#projSave").click()
            page.wait_for_function("document.getElementById('logbox').textContent.includes('save failed')")
            assert not page.evaluate("window.__calls.some(c=>c.command==='save_project')")
            step(2)
            assert page.locator(f"#domainCircle{index}").input_value() == value
            page.locator(f"#domainCircle{index}").fill(str(summary["circle"][index]))
        # Re-rendering must not round fractional sea ratio; edits invalidate old run results.
        page.evaluate("hasRun=true;runInfo={ok:true,outdir:'/old'}")
        circle = [-179, 20, 750]
        for j, value in enumerate(circle):
            page.locator(f"#domainCircle{j}").fill(str(value))
        assert page.evaluate("!hasRun && runInfo===null")
        saved = save()
        assert saved["circle"] == circle and saved["sea_ratio"] == 0.47125
        assert saved["unexposed_probe"] == "keep"
        assert saved["specified_refine_circle_count"] == summary["specified_refine_circle_count"]
        assert not page.evaluate("window.__calls.filter(c=>c.command==='preserve_unexposed_project_fields').at(-1).args.preserveDomain")
        page.evaluate("window.__opened=window.__saved")
        open_project()
        for language, width, theme in ((1, 1400, "light"), (0, 1000, "dark")):
            page.set_viewport_size({"width": width, "height": 900})
            page.evaluate("([l,t])=>{lang=l;document.documentElement.setAttribute('data-theme',t);applyI18n();}", [language, theme])
            assert [float(page.locator(f"#domainCircle{j}").input_value()) for j in range(3)] == circle
            assert float(page.locator("#seaRatioInput").input_value()) == 47.125
            assert page.locator("#work .card").evaluate("e=>e.scrollWidth<=e.clientWidth+1")
            for j in range(3):
                assert page.locator(f"#domainCircle{j}").get_attribute("aria-label")
            page.evaluate("setOlBasemap(document.getElementById('mapsvg')._olmap,'none')")
            assert page.evaluate("Array.isArray(document.getElementById('mapsvg')._olmap._domainLayer.getStyle())"), "domain needs a contrasting halo on empty/light basemaps"
            page.screenshot(path=str(source.parent / f"circle-editor-{language}.png"))
        # Radius uses the backend sphere and spherical area, not bbox area.
        area = 4 * math.pi * 6371.229**2 * math.sin(750 / 6371.229 / 2)**2
        expected = max(1, math.floor(area / (page.evaluate("cellKm()")**2 * 0.866) + 0.5))
        assert page.evaluate("estCells()") == expected
        for coordinates in (circle, [179, 20, 750], [0, 90, 500], [0, -90, 500], [35, 80, 2000], [0, 0, 10000]):
            page.evaluate("c=>{domCircle=c;drawMap();}", coordinates)
            geometry = page.evaluate("""() => {
              const map=document.getElementById('mapsvg')._olmap, ring=circleDomainRing(), r=KM_PER_DEG_EQ*180/Math.PI*1000;
              return {ring,distances:ring.map(p=>ol.sphere.getDistance(domCircle,p,r)),frame:currentOlDomainFrame(),
                plane:map._domainSource.getFeatures()[0].getGeometry().getType(),globe:globeDomainGeojson(map).features[0].geometry};
            }""")
            assert geometry["plane"] == "LineString" and geometry["globe"]["type"] == "LineString"
            assert all(math.isfinite(v) for point in geometry["ring"] for v in point)
            assert max(abs(distance - coordinates[2] * 1000) for distance in geometry["distances"]) < 0.01
            assert all(abs(b[0] - a[0]) <= 180.00001 for a, b in zip(geometry["ring"], geometry["ring"][1:]))
        page.evaluate("c=>{domCircle=c;drawMap();}", circle)
        # The detached window consumes the actual emitted settings payload.
        payload = page.evaluate("window.__events.filter(e=>e.event==='earthmesh-map-state'&&e.payload.circle).at(-1).payload")
        assert payload["circle"] == circle
        detached = browser.new_page(viewport={"width": 1000, "height": 760})
        detached.on("pageerror", lambda e: errors.append(str(e)))
        detached.route("https://**", lambda r: r.abort())
        detached.add_init_script(init)
        detached.goto(url + "?view=map&lang=en")
        detached.wait_for_function("!!window.__listeners['earthmesh-map-state']")
        detached.evaluate("payload=>window.__listeners['earthmesh-map-state']({payload:{...payload,fit:true}})", payload)
        detached.wait_for_function("document.getElementById('mapsvgModal')._olmap?._domainSource.getFeatures().length===1")
        assert detached.evaluate("domCircle") == circle
        detached.evaluate("setOlBasemap(document.getElementById('mapsvgModal')._olmap,'none')")
        detached.screenshot(path=str(source.parent / "circle-detached-plane.png"))
        detached.evaluate("setMapRenderer(document.getElementById('mapsvgModal')._olmap,'globe')")
        detached.wait_for_function("document.getElementById('mapsvgModal')._olmap._globeLoaded")
        detached.wait_for_function("document.getElementById('mapsvgModal')._olmap._globeGeoRefs.domain?.features[0]?.geometry.type==='LineString'")
        data = detached.evaluate("document.getElementById('mapsvgModal')._olmap._globe.getSource('domain').getData()")
        assert data['features'][0]['geometry']['type'] == 'LineString'
        detached.screenshot(path=str(source.parent / "circle-detached-globe.png"))
        step(6)
        page.locator("#runBtn").click()
        page.wait_for_function("window.__run && !runInProgress")
        assert page.evaluate("window.__run.circle") == circle
        assert page.evaluate("window.__run.sea_ratio") == 0.47125
        # Switching out of circle explicitly replaces it, rather than preserving hidden YAML.
        for mode, shape in (("global", "global"), ("regional", "bbox")):
            step(2)
            page.locator(f'[data-mode="{mode}"]').click()
            saved = save()
            assert saved["domain_shape"] == shape and saved["circle"] is None
        # Older backend summaries must preserve unexposed circle geometry, not silently turn it into a bbox.
        page.evaluate("window.__legacySummary=true")
        step(0)
        page.locator("#projOpen").click()
        page.wait_for_function("hiddenDomainShape==='circle'")
        step(2)
        assert page.locator("#hiddenDomainShapeText").inner_text() == "circle"
        saved = save()
        assert saved["circle"] == circle and saved["domain_shape"] == "circle"
        assert page.evaluate("window.__calls.filter(c=>c.command==='preserve_unexposed_project_fields').at(-1).args.preserveDomain")
        assert not errors, errors
        browser.close()
        server.shutdown()
    print(json.dumps({"specified_drafts": "7 required fields; inactive isolation; save/run blocking; reopen and language", "specified_chain": "read-only head preserved", "circle_roundtrip": "pass", "invalid_drafts": {"circle": 5, "bbox": 5}, "domain_modes": ["bbox", "circle", "shapefile", "close"], "picker_and_preview_ownership": "pass", "geodesic_map_cases": 6, "languages": ["zh", "en"], "viewports": [1400, 1000], "detached_plane_globe": "pass", "page_errors": errors, "transport": "mocked Tauri; real Rust summary/capability fixture"}))


if __name__ == "__main__":
    main()
