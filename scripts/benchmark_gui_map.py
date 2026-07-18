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
            page.route("https://**", lambda route: route.abort())
            page.goto(
                f"http://127.0.0.1:{server.server_port}/gui-tauri/dist/index.html?lang=en",
                wait_until="domcontentloaded",
                timeout=120_000,
            )
            result = benchmark_openlayers(page, args.fixture)
            result["mapLibreGlobe"] = benchmark_maplibre_globe(page)
            if args.fixture == "test/q_q/output/result/mesh_cells.geojson":
                assert result["features"] == result["mapLibreGlobe"]["firstLoad"]["features"] == 89_875
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
        """() => { const map=document.getElementById('mapsvg')._olmap,source=map._meshSource,
          originalGetFeatures=source.getFeatures.bind(source);let getFeaturesCalls=0;
          source.getFeatures=()=>{getFeaturesCalls+=1;return originalGetFeatures();};const started=performance.now();
          for(let i=0;i<25;i+=1)updateOlMap(map,false);
          const totalMs=performance.now()-started;source.getFeatures=originalGetFeatures;
          return {calls:25,totalMs,getFeaturesCalls,sourceFeatures:usableOlExtent(source.getExtent())?window.__benchGeojson.features.length:0}; }"""
    )
    assert unchanged["getFeaturesCalls"] == 0, unchanged
    basemap_cycle = page.evaluate(
        """() => { const map=document.getElementById('mapsvg')._olmap,source=map._meshSource,feature=source.getFeatures()[0],
          layerCount=map.getLayers().getLength(),keys=['imagery','light','topo','streets','ocean','none'],started=performance.now();
          for(let i=0;i<60;i+=1)setOlBasemap(map,keys[i%keys.length]);
          return {calls:60,totalMs:performance.now()-started,sourceStable:map._meshSource===source,
            featureStable:map._meshSource.getFeatures()[0]===feature,sourceFeatures:map._meshSource.getFeatures().length,
            layerCountStable:map.getLayers().getLength()===layerCount}; }"""
    )
    layer_toggle = page.evaluate(
        """() => { const map=document.getElementById('mapsvg')._olmap,source=map._meshSource,feature=source.getFeatures()[0],
          layers=[map._meshLayer,map._boundaryLayer,map._domainLayer,map._graticule],started=performance.now();
          for(let i=0;i<100;i+=1) layers.forEach(layer=>layer.setVisible(i%2===0));
          layers.forEach(layer=>layer.setVisible(true));
          return {changes:400,totalMs:performance.now()-started,sourceStable:map._meshSource===source,
            featureStable:map._meshSource.getFeatures()[0]===feature,sourceFeatures:map._meshSource.getFeatures().length}; }"""
    )
    assert basemap_cycle["sourceStable"] and basemap_cycle["featureStable"] and basemap_cycle["layerCountStable"]
    assert layer_toggle["sourceStable"] and layer_toggle["featureStable"]
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
        "basemapCycle": basemap_cycle,
        "layerToggle": layer_toggle,
        "projectionSwitch": projection,
        "resize": resize,
        "animatedPan": pan,
    }


def benchmark_maplibre_globe(page):
    first_load = page.evaluate(
        """async () => {
          openMapModal();
          const map=ensureOlMap('mapsvgModal');
          setOlBasemap(map,'none');
          _meshGeojson=window.__benchGeojson;_coastalGeojson=null;_domainGeojson=null;regional=false;domainMode='global';
          updateOlMap(map,false);
          const started=performance.now(),switched=setMapRenderer(map,'globe',false),syncMs=performance.now()-started;
          if(!switched||!map._globe) throw new Error('MapLibre globe did not initialize');
          await new Promise((resolve,reject)=>{
            if(map._globeLoaded) return resolve();
            const timer=setTimeout(()=>reject(new Error('MapLibre globe load timed out')),120000);
            map._globe.once('load',()=>{clearTimeout(timer);resolve();});
          });
          await waitGlobeIdle(map._globe,120000);
          const globe=map._globe,source=globe.getSource('mesh');
          const gl=globe.getCanvas().getContext('webgl2')||globe.getCanvas().getContext('webgl'),debug=gl&&gl.getExtension('WEBGL_debug_renderer_info');
          window.__benchGlobeMap=map;window.__benchGlobeInstance=globe;window.__benchGlobeSource=source;
          return {syncMs,elapsedMs:performance.now()-started,features:window.__benchGeojson.features.length,
            projection:globe.getProjection().type,loaded:map._globeLoaded,sourcePresent:!!source,
            layerCount:globe.getStyle().layers.length,canvases:map._globeContainer.querySelectorAll('canvas').length,
            gpuRenderer:gl?(debug?gl.getParameter(debug.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER)):'unavailable',contextAttributes:gl&&gl.getContextAttributes()};
        }"""
    )
    unchanged = page.evaluate(
        """() => {
          const map=window.__benchGlobeMap,globe=map._globe,source=globe.getSource('mesh'),originalSetData=source.setData;
          let setDataCalls=0;source.setData=function(data){setDataCalls+=1;return originalSetData.call(this,data)};
          const started=performance.now();for(let i=0;i<25;i+=1) updateGlobeMap(map,false);
          const totalMs=performance.now()-started;source.setData=originalSetData;
          return {calls:25,totalMs,setDataCalls,instanceStable:globe===window.__benchGlobeInstance,
            sourceStable:source===window.__benchGlobeSource,features:window.__benchGeojson.features.length};
        }"""
    )
    renderer_cycle = page.evaluate(
        """async () => {
          const map=window.__benchGlobeMap,globe=map._globe,source=globe.getSource('mesh'),originalSetData=source.setData;
          let setDataCalls=0;source.setData=function(data){setDataCalls+=1;return originalSetData.call(this,data)};
          const frame=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))),started=performance.now();
          for(let i=0;i<10;i+=1){setMapRenderer(map,'plane',false);await frame();setMapRenderer(map,'globe',false);await frame()}
          await waitGlobeIdle(globe,120000);const totalMs=performance.now()-started;source.setData=originalSetData;
          return {switches:20,totalMs,setDataCalls,instanceStable:map._globe===window.__benchGlobeInstance,
            sourceStable:map._globe.getSource('mesh')===window.__benchGlobeSource,active:map._globeActive,
            canvases:map._globeContainer.querySelectorAll('canvas').length};
        }"""
    )
    resize = page.evaluate(
        """async () => {
          const map=window.__benchGlobeMap,globe=map._globe,source=globe.getSource('mesh'),target=map._globeContainer,
            originalSetData=source.setData,oldWidth=target.style.width,oldHeight=target.style.height;
          let setDataCalls=0;source.setData=function(data){setDataCalls+=1;return originalSetData.call(this,data)};
          const started=performance.now();
          for(let i=0;i<50;i+=1){target.style.width=(600+i*8)+'px';target.style.height=(420+i*4)+'px';globe.resize()}
          const resized=[globe.getCanvas().width,globe.getCanvas().height],totalMs=performance.now()-started;
          target.style.width=oldWidth;target.style.height=oldHeight;globe.resize();
          await waitGlobeIdle(globe,120000);source.setData=originalSetData;
          return {calls:50,totalMs,setDataCalls,resized,restored:[globe.getCanvas().width,globe.getCanvas().height],
            instanceStable:globe===window.__benchGlobeInstance,sourceStable:globe.getSource('mesh')===window.__benchGlobeSource};
        }"""
    )
    interaction = page.evaluate(
        """async () => {
          const map=window.__benchGlobeMap,globe=map._globe,frames=[];let last=performance.now(),done=false;
          const ended=new Promise(resolve=>globe.once('moveend',()=>{done=true;resolve()}));
          function frame(now){frames.push(now-last);last=now;if(!done)requestAnimationFrame(frame)}
          requestAnimationFrame(frame);globe.easeTo({center:[105,24],zoom:1.5,bearing:120,pitch:0,duration:3000});
          await Promise.race([ended,new Promise(resolve=>setTimeout(resolve,6000))]);
          frames.sort((a,b)=>a-b);const q=p=>frames[Math.min(frames.length-1,Math.floor(frames.length*p))]||0;
          return {frames:frames.length,p50FrameMs:q(.5),p95FrameMs:q(.95),maxFrameMs:q(1),
            projection:globe.getProjection().type,instanceStable:globe===window.__benchGlobeInstance,
            sourceStable:globe.getSource('mesh')===window.__benchGlobeSource};
        }"""
    )
    assert first_load["loaded"] and first_load["sourcePresent"] and first_load["canvases"] == 1, first_load
    assert first_load["projection"] == "vertical-perspective", first_load
    assert unchanged["setDataCalls"] == 0 and unchanged["instanceStable"] and unchanged["sourceStable"], unchanged
    assert renderer_cycle["setDataCalls"] == 0 and renderer_cycle["instanceStable"] and renderer_cycle["sourceStable"], renderer_cycle
    assert renderer_cycle["active"] and renderer_cycle["canvases"] == 1, renderer_cycle
    assert resize["setDataCalls"] == 0 and resize["instanceStable"] and resize["sourceStable"], resize
    assert all(value > 0 for value in resize["resized"] + resize["restored"]), resize
    assert interaction["frames"] >= 3 and 0 < interaction["p50FrameMs"] <= interaction["p95FrameMs"] <= interaction["maxFrameMs"], interaction
    assert interaction["projection"] == "vertical-perspective" and interaction["instanceStable"] and interaction["sourceStable"], interaction
    return {
        "engine": "MapLibre GL JS 5.24.0 vertical-perspective",
        "firstLoad": first_load,
        "unchangedUpdate": unchanged,
        "rendererCycle": renderer_cycle,
        "resize": resize,
        "animatedGlobeInteraction": interaction,
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
