#!/usr/bin/env python3
"""Check real CLI/GUI records in Chromium with mocked Tauri transport, no solvers.

First run the opt-in Rust gui_real_project_delivery_land_atmosphere_ocean test.
Then: python3 scripts/check_gui_delivery_e2e.py /path/to/gui-records.json
Uses the existing Playwright installation; no packages are installed by this script.
"""
import json
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright


def main():
    source = Path(sys.argv[1]).resolve()
    records = json.loads(source.read_text())
    errors = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1400, "height": 900})
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.route("https://**", lambda route: route.abort())
        page.add_init_script("window.__records = " + json.dumps(records))
        page.add_init_script("""window.__calls=[]; window.__case=window.__records.cases[0];
          window.__TAURI__={webviewWindow:{WebviewWindow:{}},event:{listen:async()=>()=>{},emitTo:async()=>{}},core:{invoke:async(command,args={})=>{
            window.__calls.push({command,args});
            if(command==='project_capabilities') return window.__records.capabilities;
            if(command==='list_criteria') return [];
            if(command==='project_summary') return window.__case.summary;
            if(command==='validate_project') return [];
            if(command==='run_project') {
              if(window.__holdRun) await new Promise(resolve=>window.__finishRun=resolve);
              return window.__case.result;
            }
            if(command==='open_path') return null;
            if(command==='mesh_quality' || command==='mesh_cell_polygons') {
              const record=window.__records.cases.find(c=>c.result.gridfile===args.gridfile);
              if(!record || args.kind!==record.gui_quality.cell_view) throw new Error('wrong selected mesh/view');
              if(command==='mesh_quality') {
                if(window.__delayQuality) return new Promise((resolve,reject)=>window.__pendingQuality={resolve:()=>resolve(record.gui_quality),reject:()=>reject(new Error('delayed_previous_run'))});
                return record.gui_quality;
              }
              return JSON.stringify(record.preview);
            }
            return args.yaml || 'staged project';
          }}};""")
        page.goto((Path(__file__).resolve().parents[1] / "gui-tauri/dist/index.html").as_uri())
        page.wait_for_function("document.getElementById('logStatus').textContent.includes('Rust')")
        page.evaluate("() => {cur=6;renderStep(6);renderSteps();}")
        for case in records["cases"]:
            page.evaluate("c => {window.__case=c;window.__calls=[];regional=!!c.summary.bbox;domainMode=regional?'regional':'global';if(regional)domBbox=c.summary.bbox;}", case)
            page.locator("#runBtn").click()
            page.wait_for_function("runInfo && runInfo.outdir === window.__case.result.outdir && !runInProgress")
            page.wait_for_function("document.getElementById('qualityCells')?.textContent===String(window.__case.gui_quality.cell_count) && _meshGeojson?.features.length===window.__case.preview.features.length")
            report = case["result"]["delivery"]["report"]
            for language, width in ((1, 1400), (0, 1000)):
                page.set_viewport_size({"width": width, "height": 900})
                page.evaluate("l => {lang=l;applyI18n();}", language)
                card = page.locator("#deliveryCard")
                assert report["target"]["model_format"] in card.inner_text()
                assert report["final_quality"]["verdict"].upper() in card.inner_text()
                expected = ("仅生成通用网格" if language else "Native mesh only") if report["model_delivery_status"] == "native_only" else ("模型文件已交付" if language else "Model files delivered")
                assert expected in card.inner_text()
                card.locator("summary").click()
                paths = [report["gridfile"], report["final_quality"]["report"], case["result"]["delivery"]["report_path"], *report["model_artifacts"].values()]
                assert card.locator("button").count() == len(paths)
                for index, path in enumerate(paths):
                    assert Path(path).is_file(), path
                    card.locator("button").nth(index).focus()
                    page.keyboard.press("Enter")
                    assert page.evaluate("window.__calls.filter(c=>c.command==='open_path').at(-1).args.path") == path
                assert card.evaluate("e=>e.scrollWidth<=e.clientWidth+1"), case["name"]
                card.locator("summary").click()
                card.scroll_into_view_if_needed()
                page.screenshot(path=str(source.parent / f"{case['name']}-{language}.png"))
            assert page.evaluate("window.__calls.some(c=>c.command==='mesh_quality' && c.args.gridfile===window.__case.result.gridfile)")
            page.locator('#liveTabs button[data-pane="map"]').click()
            page.wait_for_function("document.getElementById('mapsvg')._olmap?._meshSource.getFeatures().length>0")
            cell_ids = page.evaluate("() => {const map=document.getElementById('mapsvg')._olmap;setOlBasemap(map,'none');map.renderSync();return [...new Set(map._meshSource.getFeatures().map(f=>f.get('cell_id')))];}")
            assert set(cell_ids) == {feature["properties"]["cell_id"] for feature in case["preview"]["features"]}
            page.locator("#mapsvg").screenshot(path=str(source.parent / f"{case['name']}-map.png"))
        # A previous TRI run must never replace the newer HEX run's analysis.
        for outcome in ("resolve", "reject"):
            old = next(c for c in records["cases"] if c["name"] == "land_tri")
            current = next(c for c in records["cases"] if c["name"] == "atmosphere")
            page.evaluate("c=>{window.__case=c;window.__delayQuality=true;window.__pendingQuality=null;}", old)
            page.locator("#runBtn").click()
            page.wait_for_function("!!window.__pendingQuality")
            page.evaluate("c=>{window.__case=c;window.__delayQuality=false;}", current)
            page.locator("#runBtn").click()
            page.wait_for_function("document.getElementById('qualityCells')?.textContent===String(window.__case.gui_quality.cell_count) && _meshGeojson?.features.length===window.__case.preview.features.length")
            page.evaluate("async outcome=>{window.__pendingQuality[outcome]();await new Promise(resolve=>setTimeout(resolve,50));}", outcome)
            page.screenshot(path=str(source.parent / f"late-quality-{outcome}.png"))
            assert page.locator("#qualityCells").inner_text() == str(current["gui_quality"]["cell_count"]), "previous-run quality repainted the current run"
            assert page.evaluate("_meshGeojson.features.length") == len(current["preview"]["features"]), "previous-run quality started a stale preview"
            assert "delayed_previous_run" not in page.locator("#logbox").inner_text()
        page.evaluate("window.__holdRun=true")
        page.locator("#runBtn").click()
        page.wait_for_function("!!window.__finishRun")
        page.evaluate("document.getElementById('work').scrollTop=0")
        page.screenshot(path=str(source.parent / "pending-rerun.png"))
        assert page.locator("#work .verdict").count() == 0, "pending rerun must not retain the previous DONE verdict"
        assert page.locator("#qualityCells").count() == 0, "pending rerun must not retain previous quality"
        assert page.locator("#deliveryCard button").count() == 0
        assert page.locator("#runBtn").is_disabled() and page.locator("#killBtn").is_visible()
        assert "Generating mesh" in page.locator("#work").inner_text()
        page.evaluate("() => {window.__holdRun=false;window.__finishRun();}")
        page.wait_for_function("runInfo && !runInProgress")
        # No record is unknown, failed runs cannot display stale success, and text stays inert.
        for state in ("legacy", "failed", "inert_text"):
            case = json.loads(json.dumps(records["cases"][0]))
            if state == "legacy":
                case["result"]["delivery"] = None
            elif state == "failed":
                case["result"]["ok"] = False
                case["result"]["code"] = 7  # deliberately retains stale delivery in transport
            else:
                case = json.loads(json.dumps(next(c for c in records["cases"] if c["name"] == "land_native")))
                case["result"]["delivery"]["report"]["skipped_reason"] = '<img src=x onerror="window.__injected=1">'
            page.evaluate("c=>{window.__case=c;window.__calls=[];}", case)
            page.locator("#runBtn").click()
            page.wait_for_function("runInfo && !runInProgress")
            text = page.locator("#deliveryCard").inner_text()
            if state == "legacy":
                assert "unconfirmed" in text and page.locator("#deliveryCard button").count() == 0
            elif state == "failed":
                assert "Run failed" in text and "Model files delivered" not in text
                assert not page.evaluate("window.__calls.some(c=>c.command==='mesh_quality')")
            else:
                assert "<img" in text and page.locator("#deliveryCard img").count() == 0
            page.screenshot(path=str(source.parent / f"{state}.png"))
        assert not errors, errors
        browser.close()
    print(json.dumps({"cases": len(records["cases"]), "languages": ["zh", "en"], "viewports": [1400, 1000], "legacy_failure_inert_text": "pass", "late_quality_success_error": "pass", "map_cell_ids": "match real polygons", "page_errors": errors, "transport": "mocked Tauri, real CLI/GUI delivery + quality + polygon responses"}))


if __name__ == "__main__":
    main()
