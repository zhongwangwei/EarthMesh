#!/usr/bin/env python3
"""Circle editor/round-trip/map regression, using real GUI summary fixtures.

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
            if(command==='scaffold_project') return JSON.stringify(window.__opened);
            const cfg=args.yaml?JSON.parse(args.yaml):null;
            if(command==='project_summary') {const summary={...cfg};if(window.__legacySummary)delete summary.circle;return summary;}
            if(command==='preserve_unexposed_project_fields' && args.preserveDomain){
              const old=JSON.parse(args.baseYaml);for(const key of ['domain','domain_shape','circle','bbox','sea_ratio'])cfg[key]=old[key];
            }
            if(command==='set_domain_circle')Object.assign(cfg,{domain:'regional',domain_shape:'circle',circle:[args.lon,args.lat,args.radiusKm],bbox:null,sea_ratio:args.seaRatio});
            if(command==='set_domain_bbox')Object.assign(cfg,{domain:'regional',domain_shape:'bbox',circle:null,bbox:[args.w,args.e,args.s,args.n],sea_ratio:args.seaRatio});
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

        def open_project():
            step(0)
            page.locator("#projOpen").click()
            page.wait_for_function("domainMode==='circle' && !hiddenDomainShape")
            step(2)

        def save():
            step(0)
            page.evaluate("window.__saved=null")
            page.locator("#projSave").click()
            page.wait_for_function("!!window.__saved")
            return page.evaluate("window.__saved")

        open_project()
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
    print(json.dumps({"circle_roundtrip": "pass", "invalid_drafts": 5, "geodesic_map_cases": 6, "languages": ["zh", "en"], "viewports": [1400, 1000], "detached_plane_globe": "pass", "page_errors": errors, "transport": "mocked Tauri; real Rust summary/capability fixture"}))


if __name__ == "__main__":
    main()
