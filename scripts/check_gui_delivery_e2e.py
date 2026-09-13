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
            if(command==='mesh_quality') throw new Error('headless transport: native quality recomputation not emulated');
            return args.yaml || 'staged project';
          }}};""")
        page.goto((Path(__file__).resolve().parents[1] / "gui-tauri/dist/index.html").as_uri())
        page.wait_for_function("document.getElementById('logStatus').textContent.includes('Rust')")
        page.evaluate("() => {cur=6;renderStep(6);renderSteps();}")
        for case in records["cases"]:
            page.evaluate("c => {window.__case=c;window.__calls=[];}", case)
            page.locator("#runBtn").click()
            page.wait_for_function("runInfo && runInfo.outdir === window.__case.result.outdir && !runInProgress")
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
        page.evaluate("window.__holdRun=true")
        page.locator("#runBtn").click()
        page.wait_for_function("!!window.__finishRun")
        assert page.locator("#deliveryCard").inner_text() == ""
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
    print(json.dumps({"cases": len(records["cases"]), "languages": ["zh", "en"], "viewports": [1400, 1000], "legacy_failure_inert_text": "pass", "page_errors": errors, "transport": "mocked Tauri, real CLI/GUI result records"}))


if __name__ == "__main__":
    main()
