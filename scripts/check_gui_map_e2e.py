#!/usr/bin/env python3
"""Smoke-test the independent OpenLayers map with local browser assets."""

import json
import struct
import tempfile
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from playwright.sync_api import sync_playwright


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


def main():
    root = Path(__file__).resolve().parents[1]
    server = ThreadingHTTPServer(
        ("127.0.0.1", 0),
        lambda *a, **k: QuietHandler(*a, directory=str(root), **k),
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    errors = []
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(headless=True)
            page = browser.new_page(viewport={"width": 1200, "height": 700}, accept_downloads=True)
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.route("https://server.arcgisonline.com/**", lambda route: route.abort())
            base = f"http://127.0.0.1:{server.server_port}"
            page.goto(
                f"{base}/gui-tauri/dist/index.html?view=map&lang=en",
                wait_until="domcontentloaded",
                timeout=120_000,
            )
            page.wait_for_function("document.getElementById('mapsvgModal')._olmap")
            assets = page.evaluate("performance.getEntriesByType('resource').map(entry => entry.name)")
            assert f"{base}/gui-tauri/dist/vendor/openlayers/ol.js" in assets

            dateline = page.evaluate(
                """() => {
                  const map=document.getElementById('mapsvgModal')._olmap;
                  setOlBasemap(map,'none'); regional=true; domainMode='bbox'; domBbox=[170,-170,-10,10];
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{surface_class:'land'},geometry:{type:'Polygon',coordinates:[[[179,-2],[-179,-2],[-179,2],[179,2],[179,-2]]]}}]};
                  _coastalGeojson=null; _domainGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{},geometry:{type:'Polygon',coordinates:[[[170,-10],[-170,-10],[-170,10],[170,10],[170,-10]]]}}]};
                  updateOlMap(map,true); map.renderSync();
                  const extent=map._meshSource.getExtent();
                  return {projection:olProjectionCode(map),features:map._meshSource.getFeatures().length,extent,width:extent[2]-extent[0]};
                }"""
            )
            assert dateline["projection"] == "EPSG:3857"
            assert dateline["features"] == 1
            assert 200_000 < dateline["width"] < 250_000, dateline

            page.select_option("#mapProjectionSelect", "EPSG:4326")
            geographic = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,mapExtent=map._meshSource.getExtent();map.renderSync();
                  return {projection:olProjectionCode(map),extent:mapExtent,width:mapExtent[2]-mapExtent[0],center:map.getView().getCenter()}; }"""
            )
            assert geographic["projection"] == "EPSG:4326"
            assert geographic["extent"][:2] == [179, -2]
            assert geographic["extent"][2:] == [181, 2]
            assert geographic["width"] == 2

            page.select_option("#mapProjectionSelect", "EPSG:3857")
            mercator_again = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,extent=map._meshSource.getExtent();
                  return {projection:olProjectionCode(map),width:extent[2]-extent[0]}; }"""
            )
            assert mercator_again["projection"] == "EPSG:3857"
            assert 200_000 < mercator_again["width"] < 250_000, mercator_again

            resize_results = []
            for width, height in ((600, 420), (1600, 980), (840, 1100), (1200, 600)):
                page.set_viewport_size({"width": width, "height": height})
                result = page.evaluate(
                    """async () => {
                      const target=document.getElementById('mapsvgModal'),map=target._olmap;
                      map.updateSize();map.renderSync();
                      await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
                      const tr=target.getBoundingClientRect(),vr=map.getViewport().getBoundingClientRect(),size=map.getSize();
                      const canvas=composeOlCanvas(map,Math.round(tr.width),Math.round(tr.height)),ctx=canvas.getContext('2d');
                      const bg=ctx.getImageData(2,2,1,1).data;let changed=0;
                      for(let y=12;y<canvas.height-40;y+=12) for(let x=12;x<canvas.width-12;x+=12){
                        const p=ctx.getImageData(x,y,1,1).data;
                        if(Math.abs(p[0]-bg[0])+Math.abs(p[1]-bg[1])+Math.abs(p[2]-bg[2])>24) changed+=1;
                      }
                      return {target:[tr.width,tr.height],viewport:[vr.width,vr.height],mapSize:size,changedPixels:changed};
                    }"""
                )
                assert all(abs(a - b) < 1 for a, b in zip(result["target"], result["viewport"])), result
                assert all(abs(a - b) < 1 for a, b in zip(result["target"], result["mapSize"])), result
                assert result["changedPixels"] > 10, result
                resize_results.append({"window": [width, height], **result})

            bbox_fit = page.evaluate(
                """() => {
                  const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[-125,-115,30,40];_domainGeojson=null;
                  updateOlMap(map,false);map.renderSync();
                  const domain=map._domainSource.getExtent(),view=map.getView().calculateExtent(map.getSize());
                  return {domain,view,visible:ol.extent.intersects(domain,view)};
                }"""
            )
            assert bbox_fit["visible"], bbox_fit

            tooltip_point = page.evaluate(
                """() => {
                  const target=document.getElementById('mapsvgModal'),map=target._olmap;
                  regional=true;domainMode='bbox';domBbox=[113,113.1,22,22.1];_meshGeojson=null;
                  _coastalGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{mask_class:'R3',river_class:'R3'},geometry:{type:'Polygon',coordinates:[[[113,22],[113.1,22],[113.1,22.1],[113,22.1],[113,22]]]}}]};
                  updateOlMap(map,true);map.renderSync();
                  const coordinate=ol.proj.transform([113.05,22.05],'EPSG:4326',olProjectionCode(map)),pixel=map.getPixelFromCoordinate(coordinate),rect=target.getBoundingClientRect();
                  return [rect.left+pixel[0],rect.top+pixel[1]];
                }"""
            )
            page.mouse.move(*tooltip_point)
            page.wait_for_timeout(250)
            assert not page.evaluate("document.querySelector('.ol-cell-tooltip').hidden")
            page.mouse.move(2, 2)
            page.wait_for_timeout(100)
            assert page.evaluate("document.querySelector('.ol-cell-tooltip').hidden")

            small_region = page.evaluate(
                """() => {
                  const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[113,113.1,22,22.1];
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{surface_class:'land'},geometry:{type:'Polygon',coordinates:[[[113,22],[113.1,22],[113.1,22.1],[113,22.1],[113,22]]]}}]};
                  _domainGeojson=null;_coastalGeojson=null;updateOlMap(map,false);map.setSize([1920,1080]);
                  fitOlMap(map,'region',0,[1920,1080],null);map.renderSync();
                  const extent=map._meshSource.getExtent(),left=map.getPixelFromCoordinate([extent[0],extent[1]]),right=map.getPixelFromCoordinate([extent[2],extent[1]]);
                  return {pixelWidth:Math.abs(right[0]-left[0]),zoom:map.getView().getZoom()};
                }"""
            )
            assert small_region["pixelWidth"] > 700, small_region

            page.select_option("#mapExportScope", "region")
            page.select_option("#mapExportSize", "1920x1080")
            with page.expect_download(timeout=120_000) as download_info:
                page.evaluate("saveOlMapPng(document.getElementById('mapsvgModal')._olmap)")
            download = download_info.value
            with tempfile.TemporaryDirectory() as tmpdir:
                png = Path(tmpdir) / download.suggested_filename
                download.save_as(png)
                header = png.read_bytes()[:24]
                assert header[:8] == b"\x89PNG\r\n\x1a\n" and header[12:16] == b"IHDR"
                png_size = list(struct.unpack(">II", header[16:24]))
                assert png_size == [1920, 1080], png_size

            page.select_option("#mapExportScope", "global")
            with page.expect_download(timeout=120_000) as download_info:
                page.evaluate("saveOlMapPng(document.getElementById('mapsvgModal')._olmap)")
            with tempfile.TemporaryDirectory() as tmpdir:
                png = Path(tmpdir) / download_info.value.suggested_filename
                download_info.value.save_as(png)
                global_png_size = list(struct.unpack(">II", png.read_bytes()[16:24]))
                assert global_png_size == [1920, 1080], global_png_size

            browser.close()
    finally:
        server.shutdown()

    assert not errors, errors
    print(json.dumps({"antimeridian": dateline, "geographic": geographic, "resizes": resize_results, "bboxFit": bbox_fit, "tooltipLeave": True, "smallRegion": small_region, "regionPng": png_size, "globalPng": global_png_size}, indent=2))


if __name__ == "__main__":
    main()
