#!/usr/bin/env python3
"""Benchmark EarthMesh's real map code with a real mesh GeoJSON fixture."""

import argparse
import json
import platform
import threading
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from playwright.sync_api import sync_playwright


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("fixture", nargs="?", default="test/q_q/output/result/mesh_cells.geojson")
    parser.add_argument("--output")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if not (root / args.fixture).is_file():
        raise SystemExit(f"missing real mesh fixture: {root / args.fixture}")

    server = ThreadingHTTPServer(
        ("127.0.0.1", 0),
        lambda *a, **k: QuietHandler(*a, directory=str(root), **k),
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(headless=True)
            page = browser.new_page(viewport={"width": 1400, "height": 900})
            page.route("https://server.arcgisonline.com/**", lambda route: route.abort())
            page.goto(
                f"http://127.0.0.1:{server.server_port}/gui-tauri/dist/index.html?lang=en",
                wait_until="domcontentloaded",
                timeout=120_000,
            )
            result = benchmark_openlayers(page, args.fixture)
            result.update(
                fixture=args.fixture,
                browser=browser.version,
                platform=platform.platform(),
                viewport=[1400, 900],
            )
            browser.close()
    finally:
        server.shutdown()

    text = json.dumps(result, indent=2) + "\n"
    if args.output:
        output = root / args.output
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text)
    print(text, end="")


def benchmark_openlayers(page, fixture):
    page.wait_for_function("document.getElementById('mapsvg')._olmap")
    parsed = load_fixture(page, fixture)
    first_layer = page.evaluate(
        """async () => {
          const map=document.getElementById('mapsvg')._olmap; setOlBasemap(map,'none');
          _meshGeojson=window.__benchGeojson;_coastalGeojson=null;_domainGeojson=null;regional=false;domainMode='global';
          const started=performance.now();updateOlMap(map,true);const syncMs=performance.now()-started;
          await new Promise(resolve=>map.once('rendercomplete',resolve));map.renderSync();
          return {syncMs,elapsedMs:performance.now()-started,sourceFeatures:map._meshSource.getFeatures().length,
            layerCount:map.getLayers().getLength(),canvases:map.getViewport().querySelectorAll('canvas').length};
        }"""
    )
    unchanged = page.evaluate(
        """() => { const map=document.getElementById('mapsvg')._olmap,started=performance.now();
          for(let i=0;i<25;i+=1)updateOlMap(map,false);
          return {calls:25,totalMs:performance.now()-started,sourceFeatures:map._meshSource.getFeatures().length}; }"""
    )
    projection = page.evaluate(
        """async () => { const map=document.getElementById('mapsvg')._olmap,started=performance.now();
          changeOlProjection(map,'EPSG:4326');const syncMs=performance.now()-started;
          await new Promise(resolve=>map.once('rendercomplete',resolve));map.renderSync();
          return {syncMs,elapsedMs:performance.now()-started,projection:olProjectionCode(map),features:map._meshSource.getFeatures().length}; }"""
    )
    resize = page.evaluate(
        """async () => { const map=document.getElementById('mapsvg')._olmap,started=performance.now();
          for(let i=0;i<50;i+=1){map.setSize([800+i*4,600+i*2]);map.updateSize()}
          await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
          return {calls:50,totalMs:performance.now()-started}; }"""
    )
    pan = page.evaluate(
        """async () => { const map=document.getElementById('mapsvg')._olmap,view=map.getView(),frames=[];
          let last=performance.now(),done=false;const ended=new Promise(resolve=>map.once('moveend',()=>{done=true;resolve();}));
          function frame(now){frames.push(now-last);last=now;if(!done)requestAnimationFrame(frame)}
          const center=view.getCenter();requestAnimationFrame(frame);view.animate({center:[center[0]+20,center[1]],duration:500});
          await Promise.race([ended,new Promise(resolve=>setTimeout(resolve,2000))]);
          frames.sort((a,b)=>a-b);const q=p=>frames[Math.min(frames.length-1,Math.floor(frames.length*p))]||0;
          return {frames:frames.length,p50FrameMs:q(.5),p95FrameMs:q(.95),maxFrameMs:q(1)}; }"""
    )
    return {
        "engine": "OpenLayers 10.9.0 VectorImage",
        **parsed,
        "firstLayer": first_layer,
        "unchangedUpdate": unchanged,
        "projectionSwitch": projection,
        "resize": resize,
        "animatedPan": pan,
    }


def load_fixture(page, fixture):
    return page.evaluate(
        """async path => { const text=await(await fetch('/'+path)).text(),started=performance.now();
          window.__benchGeojson=JSON.parse(text);
          return {bytes:text.length,features:window.__benchGeojson.features.length,parseMs:performance.now()-started}; }""",
        fixture,
    )


if __name__ == "__main__":
    main()
