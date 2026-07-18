#!/usr/bin/env python3
"""Smoke-test the independent plane and globe maps with local browser assets."""

import json
import math
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
            page.route("https://**", lambda route: route.abort())
            base = f"http://127.0.0.1:{server.server_port}"
            page.goto(
                f"{base}/gui-tauri/dist/index.html?view=map&lang=en",
                wait_until="domcontentloaded",
                timeout=120_000,
            )
            page.wait_for_function("document.getElementById('mapsvgModal')._olmap")
            assets = page.evaluate("performance.getEntriesByType('resource').map(entry => entry.name)")
            assert f"{base}/gui-tauri/dist/vendor/openlayers/ol.js" in assets
            assert f"{base}/gui-tauri/dist/vendor/maplibre/maplibre-gl-csp.js" in assets
            assert f"{base}/gui-tauri/dist/vendor/maplibre/maplibre-gl.css" in assets

            menu_bounds = {}
            for language, width in (("zh", 700), ("en", 1000)):
                page.set_viewport_size({"width": width, "height": 700})
                page.evaluate("language => { lang=language==='zh'?1:0; applyI18n(); }", language)
                menu_bounds[language] = {}
                for menu_id in ("mapLayersMenu", "mapMeasureMenu", "mapExportMenu"):
                    page.locator(f"#{menu_id}").evaluate("menu => { menu.open=true; }")
                    page.wait_for_timeout(20)
                    bounds = page.locator(f"#{menu_id} .map-tool-panel").evaluate(
                        "panel => { const rect=panel.getBoundingClientRect(); return {left:rect.left,right:rect.right}; }"
                    )
                    assert bounds["left"] >= -0.5 and bounds["right"] <= width + 0.5, (language, width, menu_id, bounds)
                    menu_bounds[language][menu_id] = bounds
                    page.locator(f"#{menu_id}").evaluate("menu => { menu.open=false; }")
            page.set_viewport_size({"width": 1200, "height": 700})
            page.evaluate("() => { lang=0; applyI18n(); }")

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
            assert page.locator('#mapProjectionSelect option[value="UTM:AUTO"]').evaluate("option => option.disabled")
            assert page.evaluate("[olUtmZone(6,60),olUtmZone(15,78),olUtmZone(-105,40)]") == [32, 33, 13]

            page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  window.__mapControlMeshSource=map._meshSource;
                  window.__mapControlMeshFeature=map._meshSource.getFeatures()[0];
                  window.__mapControlLayerCount=map.getLayers().getLength(); }"""
            )
            basemap_results = {}
            for key in ("imagery", "light", "topo", "streets", "ocean", "none"):
                page.select_option("#mapBasemapSelect", key)
                state = page.evaluate(
                    """key => { const map=document.getElementById('mapsvgModal')._olmap,cfg=OL_BASEMAPS[key];
                      return {key:map._basemapKey,visible:map._baseLayer.getVisible(),hasSource:!!map._baseLayer.getSource(),
                        attribution:cfg.attribution,sourceStable:map._meshSource===window.__mapControlMeshSource,
                        featureStable:map._meshSource.getFeatures()[0]===window.__mapControlMeshFeature,
                        layerCount:map.getLayers().getLength()}; }""",
                    key,
                )
                assert state["key"] == key, state
                assert state["visible"] == (key != "none"), state
                assert state["hasSource"] == (key != "none"), state
                assert bool(state["attribution"]) == (key != "none"), state
                assert state["sourceStable"] and state["featureStable"], state
                assert state["layerCount"] == page.evaluate("window.__mapControlLayerCount"), state
                basemap_results[key] = state

            page.locator("#mapLayersMenu").evaluate("element => element.open=true")
            layer_targets = {
                "mapMeshVisible": "_meshLayer",
                "mapBoundaryVisible": "_boundaryLayer",
                "mapDomainVisible": "_domainLayer",
                "mapGraticuleVisible": "_graticule",
            }
            for control, layer_name in layer_targets.items():
                page.check(f"#{control}")
                assert page.evaluate(
                    "([name]) => document.getElementById('mapsvgModal')._olmap[name].getVisible()",
                    [layer_name],
                )
                page.uncheck(f"#{control}")
                assert not page.evaluate(
                    "([name]) => document.getElementById('mapsvgModal')._olmap[name].getVisible()",
                    [layer_name],
                )
                page.check(f"#{control}")

            assert page.locator("#mapLegendVisible").evaluate("control => control.disabled")
            page.locator("#mapOpacity").fill("47")
            page.locator("#mapBaseOpacity").fill("38")
            control_state = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  return {meshOpacity:map._meshLayer.getOpacity(),baseOpacity:map._baseLayer.getOpacity(),
                    meshOutput:document.getElementById('mapOpacityValue').textContent,
                    baseOutput:document.getElementById('mapBaseOpacityValue').textContent,
                    sourceStable:map._meshSource===window.__mapControlMeshSource,
                    featureStable:map._meshSource.getFeatures()[0]===window.__mapControlMeshFeature,
                    layerCount:map.getLayers().getLength()}; }"""
            )
            assert abs(control_state["meshOpacity"] - 0.47) < 1e-9, control_state
            assert abs(control_state["baseOpacity"] - 0.38) < 1e-9, control_state
            assert control_state["meshOutput"] == "47%" and control_state["baseOutput"] == "38%", control_state
            assert control_state["sourceStable"] and control_state["featureStable"], control_state
            assert control_state["layerCount"] == page.evaluate("window.__mapControlLayerCount"), control_state

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

            page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[112,114,21,23];
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{surface_class:'land'},geometry:{type:'Polygon',coordinates:[[[112.9,21.9],[113.1,21.9],[113.1,22.1],[112.9,22.1],[112.9,21.9]]]}}]};
                  _coastalGeojson=null;_domainGeojson=null;updateOlMap(map,true);map.renderSync(); }"""
            )
            assert not page.locator('#mapProjectionSelect option[value="UTM:AUTO"]').evaluate("option => option.disabled")
            page.select_option("#mapProjectionSelect", "UTM:AUTO")
            utm_north = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,extent=map._meshSource.getExtent(),projection=olProjectionCode(map);
                  return {projection,choice:map._projectionChoice,features:map._meshSource.getFeatures().length,extent,
                    center4326:ol.proj.transform(map.getView().getCenter(),projection,'EPSG:4326')}; }"""
            )
            assert utm_north["projection"] == "EPSG:32649", utm_north
            assert utm_north["choice"] == "UTM:AUTO" and utm_north["features"] == 1, utm_north
            assert all(math.isfinite(value) for value in utm_north["extent"]), utm_north
            assert abs(utm_north["center4326"][0] - 113) < 0.2 and abs(utm_north["center4326"][1] - 22) < 0.2, utm_north

            utm_south = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[17,19,-34,-32];
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{surface_class:'land'},geometry:{type:'Polygon',coordinates:[[[17.9,-33.1],[18.1,-33.1],[18.1,-32.9],[17.9,-32.9],[17.9,-33.1]]]}}]};
                  updateOlMap(map,true);map.renderSync();const projection=olProjectionCode(map),extent=map._meshSource.getExtent();
                  return {projection,choice:map._projectionChoice,features:map._meshSource.getFeatures().length,extent,
                    center4326:ol.proj.transform(map.getView().getCenter(),projection,'EPSG:4326')}; }"""
            )
            assert utm_south["projection"] == "EPSG:32734", utm_south
            assert utm_south["choice"] == "UTM:AUTO" and utm_south["features"] == 1, utm_south
            assert all(math.isfinite(value) for value in utm_south["extent"]), utm_south
            assert abs(utm_south["center4326"][0] - 18) < 0.2 and abs(utm_south["center4326"][1] + 33) < 0.2, utm_south
            page.select_option("#mapProjectionSelect", "EPSG:3857")

            watershed_frame = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='watershed';
                  _domainGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{},geometry:{type:'Polygon',coordinates:[[[7,46],[11,46],[11,50],[7,50],[7,46]]]}}]};
                  _meshGeojson=null;_coastalGeojson=null;updateOlMap(map,true);map.renderSync();
                  return {frame:currentOlDomainFrame(),disabled:document.querySelector('#mapProjectionSelect option[value="UTM:AUTO"]').disabled}; }"""
            )
            assert watershed_frame["frame"] == {"west": 7, "east": 11, "south": 46, "north": 50, "crossesDateline": False}, watershed_frame
            assert not watershed_frame["disabled"], watershed_frame
            page.select_option("#mapProjectionSelect", "UTM:AUTO")
            assert page.evaluate("olProjectionCode(document.getElementById('mapsvgModal')._olmap)") == "EPSG:32632"
            page.select_option("#mapProjectionSelect", "EPSG:3857")

            page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[3,12,59,61];_domainGeojson=null;_meshGeojson=null;_coastalGeojson=null;
                  updateOlMap(map,true);map.renderSync(); }"""
            )
            page.select_option("#mapProjectionSelect", "UTM:AUTO")
            norway_utm = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,projection=map.getView().getProjection();
                  fitOlMap(map,'global',0);const extent=projection.getExtent(),worldExtent=projection.getWorldExtent();
                  const west=ol.proj.transform([3,60],'EPSG:4326',projection),east=ol.proj.transform([12,60],'EPSG:4326',projection);
                  return {projection:projection.getCode(),worldExtent,extent,westCovered:ol.extent.containsCoordinate(extent,west),
                    eastCovered:ol.extent.containsCoordinate(extent,east),layerExtents:map.getLayers().getArray().map(layer=>layer.getExtent())}; }"""
            )
            assert norway_utm["projection"] == "EPSG:32632" and norway_utm["worldExtent"][:3:2] == [3, 12], norway_utm
            assert norway_utm["westCovered"] and norway_utm["eastCovered"], norway_utm
            assert all(extent == norway_utm["extent"] for extent in norway_utm["layerExtents"]), norway_utm
            page.select_option("#mapProjectionSelect", "EPSG:3857")

            page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[9,21,77,79];updateOlMap(map,true);map.renderSync(); }"""
            )
            page.select_option("#mapProjectionSelect", "UTM:AUTO")
            svalbard_utm = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,projection=map.getView().getProjection();
                  fitOlMap(map,'global',0);const extent=projection.getExtent(),worldExtent=projection.getWorldExtent();
                  const west=ol.proj.transform([9,78],'EPSG:4326',projection),east=ol.proj.transform([21,78],'EPSG:4326',projection);
                  return {projection:projection.getCode(),worldExtent,extent,westCovered:ol.extent.containsCoordinate(extent,west),
                    eastCovered:ol.extent.containsCoordinate(extent,east),layerExtents:map.getLayers().getArray().map(layer=>layer.getExtent())}; }"""
            )
            assert svalbard_utm["projection"] == "EPSG:32633" and svalbard_utm["worldExtent"][:3:2] == [9, 21], svalbard_utm
            assert svalbard_utm["westCovered"] and svalbard_utm["eastCovered"], svalbard_utm
            assert all(extent == svalbard_utm["extent"] for extent in svalbard_utm["layerExtents"]), svalbard_utm
            page.select_option("#mapProjectionSelect", "EPSG:3857")

            close_empty = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='close';_domainGeojson=null;_meshGeojson=null;_coastalGeojson=null;
                  updateOlMap(map,true);map.renderSync();
                  return {frame:currentOlDomainFrame(),disabled:document.querySelector('#mapProjectionSelect option[value="UTM:AUTO"]').disabled,
                    mesh:map._meshSource.getFeatures().length,boundary:map._boundarySource.getFeatures().length}; }"""
            )
            assert close_empty == {"frame": None, "disabled": True, "mesh": 0, "boundary": 0}, close_empty
            page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{surface_class:'land'},geometry:{type:'Polygon',coordinates:[[[-105.2,39.8],[-104.8,39.8],[-104.8,40.2],[-105.2,40.2],[-105.2,39.8]]]}}]};
                  updateOlMap(map,true);map.renderSync(); }"""
            )
            assert not page.locator('#mapProjectionSelect option[value="UTM:AUTO"]').evaluate("option => option.disabled")
            page.select_option("#mapProjectionSelect", "UTM:AUTO")
            close_mesh_utm = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  return {projection:olProjectionCode(map),frame:currentOlDomainFrame(),features:map._meshSource.getFeatures().length}; }"""
            )
            assert close_mesh_utm["projection"] == "EPSG:32613" and close_mesh_utm["features"] == 1, close_mesh_utm
            assert abs(close_mesh_utm["frame"]["east"] - close_mesh_utm["frame"]["west"] - 0.4) < 1e-9, close_mesh_utm
            page.select_option("#mapProjectionSelect", "EPSG:3857")

            watershed_dateline = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='watershed';_meshGeojson=null;_coastalGeojson=null;
                  _domainGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{},geometry:{type:'Polygon',coordinates:[[[179.5,-2],[-179.5,-2],[-179.5,2],[179.5,2],[179.5,-2]]]}}]};
                  updateOlMap(map,true);map.renderSync();const frame=currentOlDomainFrame(),extent=map._boundarySource.getExtent();
                  return {frame,width:extent[2]-extent[0],disabled:document.querySelector('#mapProjectionSelect option[value="UTM:AUTO"]').disabled}; }"""
            )
            assert watershed_dateline["frame"]["crossesDateline"], watershed_dateline
            assert abs(watershed_dateline["frame"]["east"] - watershed_dateline["frame"]["west"] - 1) < 1e-9, watershed_dateline
            assert 100_000 < watershed_dateline["width"] < 120_000 and watershed_dateline["disabled"], watershed_dateline

            empty_projection = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=false;domainMode='global';_meshGeojson=null;_domainGeojson=null;_coastalGeojson=null;
                  updateOlMap(map,false);fitOlMap(map,'global',0);changeOlProjection(map,'EPSG:4326');map.renderSync();
                  const projectionExtent=map.getView().getProjection().getExtent();
                  return {projection:olProjectionCode(map),features:map._meshSource.getFeatures().length,projectionExtent,
                    layerExtents:map.getLayers().getArray().map(layer=>layer.getExtent())}; }"""
            )
            assert empty_projection["projection"] == "EPSG:4326" and empty_projection["features"] == 0, empty_projection
            assert all(extent == empty_projection["projectionExtent"] for extent in empty_projection["layerExtents"]), empty_projection
            page.select_option("#mapProjectionSelect", "EPSG:3857")

            inspector_replacement = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  regional=true;domainMode='bbox';domBbox=[10,11,45,46];_domainGeojson=null;_coastalGeojson=null;
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{cell_id:'old'},geometry:{type:'Polygon',coordinates:[[[10,45],[11,45],[11,46],[10,46],[10,45]]]}}]};
                  updateOlMap(map,true);showOlCellInspector(map,map._meshSource.getFeatures()[0]);
                  const before={hidden:map._cellInspectorElement.hidden,text:map._cellInspectorBody.textContent};
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{cell_id:'new'},geometry:{type:'Polygon',coordinates:[[[10.2,45.2],[10.8,45.2],[10.8,45.8],[10.2,45.8],[10.2,45.2]]]}}]};
                  updateOlMap(map,false);return {before,after:{hidden:map._cellInspectorElement.hidden,children:map._cellInspectorBody.children.length}}; }"""
            )
            assert not inspector_replacement["before"]["hidden"] and "old" in inspector_replacement["before"]["text"], inspector_replacement
            assert inspector_replacement["after"] == {"hidden": True, "children": 0}, inspector_replacement

            resize_results = []
            for width, height in ((600, 420), (1600, 980), (840, 1100), (1200, 600)):
                page.set_viewport_size({"width": width, "height": height})
                result = page.evaluate(
                    """async () => {
                      const target=document.getElementById('mapsvgModal'),map=target._olmap;
                      map.updateSize();fitOlMap(map,'region',0);map.renderSync();
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

            world_before = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  fitOlMap(map,'region',0);map.renderSync();window.__worldMeshSource=map._meshSource;
                  window.__worldMeshFeature=map._meshSource.getFeatures()[0];
                  const extent=map.getView().calculateExtent(map.getSize());return extent[2]-extent[0]; }"""
            )
            page.click("#mapWorldBtn")
            page.wait_for_timeout(350)
            world_view = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,extent=map.getView().calculateExtent(map.getSize());
                  return {width:extent[2]-extent[0],sourceStable:map._meshSource===window.__worldMeshSource,
                    featureStable:map._meshSource.getFeatures()[0]===window.__worldMeshFeature}; }"""
            )
            assert world_view["width"] > world_before * 2, (world_before, world_view)
            assert world_view["sourceStable"] and world_view["featureStable"], world_view

            world_margins = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,projectionExtent=map.getView().getProjection().getExtent(),
                  viewExtent=map.getView().calculateExtent(map.getSize()),feature=map._meshSource.getFeatures()[0],results=[];
                  const coordinates=[[(viewExtent[0]+projectionExtent[0])/2,0],[(viewExtent[2]+projectionExtent[2])/2,0]];
                  coordinates.forEach((coordinate)=>{
                    const pixel=map.getPixelFromCoordinate(coordinate);
                    map._cellTooltipElement.hidden=false;showOlCellInspector(map,feature);
                    map.dispatchEvent({type:'pointermove',coordinate,pixel,dragging:false});const tooltipHidden=map._cellTooltipElement.hidden;
                    showOlCellInspector(map,feature);map.dispatchEvent({type:'singleclick',coordinate,pixel});
                    results.push({pixel,tooltipHidden,inspectorHidden:map._cellInspectorElement.hidden});
                  });
                  return {worldClip:map._worldClip,projectionExtent,viewExtent,results}; }"""
            )
            assert world_margins["worldClip"], world_margins
            assert world_margins["viewExtent"][0] < world_margins["projectionExtent"][0], world_margins
            assert world_margins["viewExtent"][2] > world_margins["projectionExtent"][2], world_margins
            assert all(item["tooltipHidden"] and item["inspectorHidden"] for item in world_margins["results"]), world_margins

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

            page.locator("#mapLayersMenu").evaluate("element => element.open=true")
            page.uncheck("#mapMeshVisible")
            page.mouse.move(*tooltip_point)
            page.mouse.click(*tooltip_point)
            page.wait_for_timeout(250)
            mesh_hidden_hit_test = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  return {tooltipHidden:map._cellTooltipElement.hidden,inspectorHidden:map._cellInspectorElement.hidden}; }"""
            )
            assert mesh_hidden_hit_test == {"tooltipHidden": True, "inspectorHidden": True}, mesh_hidden_hit_test
            page.check("#mapMeshVisible")

            page.locator("#mapMeasureMenu").evaluate("element => element.open=true")
            page.select_option("#mapMeasureMode", "distance")
            measure_active = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  const geometry=new ol.geom.LineString([[0,0],[1000,0]]);
                  map._measureSource.addFeature(new ol.Feature(geometry));
                  document.getElementById('mapMeasureStatus').textContent=formatOlMeasure(geometry,map);
                  return {mode:map._measureMode,interaction:!!map._measureInteraction,count:map._measureSource.getFeatures().length,
                    status:document.getElementById('mapMeasureStatus').textContent}; }"""
            )
            assert measure_active["mode"] == "distance" and measure_active["interaction"], measure_active
            assert measure_active["count"] == 1 and measure_active["status"], measure_active
            page.click("#mapMeasureClearBtn")
            measure_cleared = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  return {count:map._measureSource.getFeatures().length,status:document.getElementById('mapMeasureStatus').textContent}; }"""
            )
            assert measure_cleared == {"count": 0, "status": ""}, measure_cleared
            page.select_option("#mapMeasureMode", "none")
            assert page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  return map._measureMode==='none'&&!map._measureInteraction; }"""
            )

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

            page.locator("#mapExportMenu").evaluate("element => element.open=true")
            page.select_option("#mapExportScope", "region")
            page.select_option("#mapExportSize", "1920x1080")
            export_controls_before = page.evaluate(
                """() => Array.from(document.querySelectorAll('#mapTools button,#mapTools select,#mapTools input'))
                  .map(control=>({id:control.id,disabled:control.disabled}))"""
            )
            page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  window.__realWaitOlRender=waitOlRender;window.__releaseMapExport=null;
                  waitOlRender=()=>new Promise(resolve=>{window.__releaseMapExport=resolve;});
                  window.__delayedMapExport=saveOlMapPng(map).finally(()=>{waitOlRender=window.__realWaitOlRender;}); }"""
            )
            page.wait_for_function("typeof window.__releaseMapExport==='function'")
            export_controls_pending = page.evaluate(
                """() => Array.from(document.querySelectorAll('#mapTools button,#mapTools select,#mapTools input'))
                  .map(control=>({id:control.id,disabled:control.disabled}))"""
            )
            assert export_controls_pending and all(item["disabled"] for item in export_controls_pending), export_controls_pending
            with page.expect_download(timeout=120_000):
                page.evaluate("window.__releaseMapExport()")
            page.evaluate("window.__delayedMapExport")
            export_controls_after = page.evaluate(
                """() => Array.from(document.querySelectorAll('#mapTools button,#mapTools select,#mapTools input'))
                  .map(control=>({id:control.id,disabled:control.disabled}))"""
            )
            assert export_controls_after == export_controls_before, (export_controls_before, export_controls_after)

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

            page.set_viewport_size({"width": 1200, "height": 700})
            page.evaluate(
                """() => {
                  const map=document.getElementById('mapsvgModal')._olmap;
                  setOlBasemap(map,'none'); regional=false; domainMode='global';
                  _meshGeojson={type:'FeatureCollection',features:[{type:'Feature',properties:{cell_id:'globe-cell',surface_class:'land'},geometry:{type:'Polygon',coordinates:[[[-20,-20],[20,-20],[20,20],[-20,20],[-20,-20]]]}}]};
                  _domainGeojson=null;_coastalGeojson=null;updateOlMap(map,true);map.renderSync();
                  window.__globeRaw=_meshGeojson;window.__globePlaneSource=map._meshSource;
                  window.__globePlaneFeature=map._meshSource.getFeatures()[0];
                }"""
            )
            page.select_option("#mapRendererSelect", "globe")
            page.wait_for_function(
                "document.getElementById('mapsvgModal')._olmap._globeLoaded",
                timeout=120_000,
            )
            page.evaluate("waitGlobeIdle(document.getElementById('mapsvgModal')._olmap._globe)")
            globe_initial = page.evaluate(
                """() => {
                  const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe,source=globe.getSource('mesh');
                  window.__globeMap=globe;window.__globeSource=source;window.__globeSetDataCalls=0;
                  const setData=source.setData.bind(source);
                  source.setData=(data)=>{window.__globeSetDataCalls+=1;return setData(data);};
                  const projectionControl=document.getElementById('mapProjectionSelect');
                  return {active:map._globeActive,loaded:map._globeLoaded,renderer:document.getElementById('mapRendererSelect').value,
                    projection:globe.getProjection().type,projectionValue:projectionControl.value,projectionDisabled:projectionControl.disabled,
                    features:_meshGeojson.features.length,
                    workerUrl:maplibregl.getWorkerUrl(),canvases:document.querySelectorAll('#mapglobeModal canvas.maplibregl-canvas').length,
                    planeHidden:map.getTargetElement().hidden,globeHidden:map._globeContainer.hidden};
                }"""
            )
            assert globe_initial == {
                "active": True,
                "loaded": True,
                "renderer": "globe",
                "projection": "vertical-perspective",
                "projectionValue": "GLOBE",
                "projectionDisabled": True,
                "features": 1,
                "workerUrl": f"{base}/gui-tauri/dist/vendor/maplibre/maplibre-gl-csp-worker.js",
                "canvases": 1,
                "planeHidden": True,
                "globeHidden": False,
            }, globe_initial

            globe_identity = page.evaluate(
                """async () => {
                  const map=document.getElementById('mapsvgModal')._olmap;
                  for(let i=0;i<3;i+=1){
                    setMapRenderer(map,'plane',false);await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
                    setMapRenderer(map,'globe',false);await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
                  }
                  for(let i=0;i<25;i+=1) updateGlobeMap(map,false);
                  await waitGlobeIdle(map._globe);
                  return {mapStable:map._globe===window.__globeMap,sourceStable:map._globe.getSource('mesh')===window.__globeSource,
                    rawStable:_meshGeojson===window.__globeRaw,setDataCalls:window.__globeSetDataCalls,
                    planeSourceStable:map._meshSource===window.__globePlaneSource,
                    planeFeatureStable:map._meshSource.getFeatures()[0]===window.__globePlaneFeature,
                    canvases:document.querySelectorAll('#mapglobeModal canvas.maplibregl-canvas').length,
                    active:map._globeActive,planeHidden:map.getTargetElement().hidden,globeHidden:map._globeContainer.hidden};
                }"""
            )
            assert globe_identity == {
                "mapStable": True,
                "sourceStable": True,
                "rawStable": True,
                "setDataCalls": 0,
                "planeSourceStable": True,
                "planeFeatureStable": True,
                "canvases": 1,
                "active": True,
                "planeHidden": True,
                "globeHidden": False,
            }, globe_identity

            page.evaluate(
                """() => { const globe=document.getElementById('mapsvgModal')._olmap._globe;
                  window.__globeResizeCalls=0;window.__globeResize=globe.resize.bind(globe);
                  globe.resize=()=>{window.__globeResizeCalls+=1;return window.__globeResize();}; }"""
            )
            globe_resizes = []
            for width, height in ((700, 520), (1500, 950)):
                page.evaluate("window.__globeResizeCalls=0")
                page.set_viewport_size({"width": width, "height": height})
                resized = page.evaluate(
                    """async () => {
                      const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe,target=map._globeContainer;
                      await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
                      fitGlobeMap(map,'global',0);await waitGlobeIdle(globe);
                      const tr=target.getBoundingClientRect(),cr=globe.getCanvas().getBoundingClientRect();
                      const canvas=composeGlobeCanvas(map,Math.round(tr.width),Math.round(tr.height)),ctx=canvas.getContext('2d');
                      const bg=ctx.getImageData(2,2,1,1).data;let changed=0,minX=canvas.width,minY=canvas.height,maxX=-1,maxY=-1;
                      for(let y=8;y<canvas.height-36;y+=8) for(let x=8;x<canvas.width-8;x+=8){
                        const p=ctx.getImageData(x,y,1,1).data;
                        if(Math.abs(p[0]-bg[0])+Math.abs(p[1]-bg[1])+Math.abs(p[2]-bg[2])>18){changed+=1;minX=Math.min(minX,x);minY=Math.min(minY,y);maxX=Math.max(maxX,x);maxY=Math.max(maxY,y);}
                      }
                      return {target:[tr.width,tr.height],canvas:[cr.width,cr.height],buffer:[globe.getCanvas().width,globe.getCanvas().height],
                        changedPixels:changed,content:[maxX-minX,maxY-minY],resizeCalls:window.__globeResizeCalls,sourceStable:globe.getSource('mesh')===window.__globeSource,
                        setDataCalls:window.__globeSetDataCalls};
                    }"""
                )
                assert all(abs(a - b) < 1 for a, b in zip(resized["target"], resized["canvas"])), resized
                assert all(a >= b for a, b in zip(resized["buffer"], resized["canvas"])), resized
                assert resized["changedPixels"] > 100, resized
                assert min(resized["content"]) > min(resized["target"]) * 0.4, resized
                assert resized["resizeCalls"] == 1, resized
                assert resized["sourceStable"] and resized["setDataCalls"] == 0, resized
                globe_resizes.append({"window": [width, height], **resized})

            page.set_viewport_size({"width": 1200, "height": 700})
            globe_antimeridian = page.evaluate(
                """async () => {
                  const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe,original=window.__globeRaw;
                  await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
                  const raw={type:'FeatureCollection',features:[{type:'Feature',properties:{cell_id:'dateline-cell'},geometry:{type:'Polygon',coordinates:[[[179,-2],[-179,-2],[-179,2],[179,2],[179,-2]]]}}]};
                  regional=true;domainMode='bbox';domBbox=[170,-170,-10,10];_meshGeojson=raw;updateGlobeMap(map,true);await waitGlobeIdle(globe);
                  const rendered=globeGeojson(raw),query=()=>globe.queryRenderedFeatures(globe.project(globe.getCenter()),{layers:['earthmesh-mesh-fill']}).map(feature=>feature.properties.cell_id);
                  globe.jumpTo({center:[0,0],zoom:2,bearing:0,pitch:0});await waitGlobeIdle(globe);const greenwich=query();
                  globe.jumpTo({center:[180,0],zoom:2,bearing:0,pitch:0});await waitGlobeIdle(globe);const dateline=query();
                  _meshGeojson=original;regional=false;domainMode='global';updateGlobeMap(map,true);await waitGlobeIdle(globe);
                  const setDataCalls=window.__globeSetDataCalls;window.__globeSetDataCalls=0;
                  return {rawLongitude:raw.features[0].geometry.coordinates[0][1][0],renderedLongitude:rendered.features[0].geometry.coordinates[0][1][0],
                    cached:globeGeojson(raw)===rendered,greenwich,dateline,setDataCalls,sourceStable:globe.getSource('mesh')===window.__globeSource};
                }"""
            )
            assert globe_antimeridian["rawLongitude"] == -179 and globe_antimeridian["renderedLongitude"] == 181, globe_antimeridian
            assert globe_antimeridian["cached"] and not globe_antimeridian["greenwich"], globe_antimeridian
            assert "dateline-cell" in globe_antimeridian["dateline"], globe_antimeridian
            assert globe_antimeridian["setDataCalls"] == 2 and globe_antimeridian["sourceStable"], globe_antimeridian

            globe_hit = page.evaluate(
                """async () => {
                  const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe;
                  globe.jumpTo({center:[0,0],zoom:2,bearing:0,pitch:0});await waitGlobeIdle(globe);
                  const point=globe.project([0,0]),rect=globe.getCanvas().getBoundingClientRect();
                  const features=globe.queryRenderedFeatures(point,{layers:['earthmesh-mesh-fill']});
                  return {point:[rect.left+point.x,rect.top+point.y],cellId:features[0]&&features[0].properties.cell_id};
                }"""
            )
            assert globe_hit["cellId"] == "globe-cell", globe_hit
            page.mouse.click(*globe_hit["point"])
            page.wait_for_timeout(50)
            globe_inspector = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap;
                  return {hidden:map._cellInspectorElement.hidden,text:map._cellInspectorBody.textContent}; }"""
            )
            assert not globe_inspector["hidden"] and "globe-cell" in globe_inspector["text"], globe_inspector
            page.locator("#mapLayersMenu").evaluate("element => element.open=true")
            page.uncheck("#mapMeshVisible")
            globe_hidden = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe;
                  return {fill:globe.getLayoutProperty('earthmesh-mesh-fill','visibility'),line:globe.getLayoutProperty('earthmesh-mesh-line','visibility'),
                    inspectorHidden:map._cellInspectorElement.hidden,sourceStable:globe.getSource('mesh')===window.__globeSource,
                    setDataCalls:window.__globeSetDataCalls}; }"""
            )
            assert globe_hidden == {
                "fill": "none",
                "line": "none",
                "inspectorHidden": True,
                "sourceStable": True,
                "setDataCalls": 0,
            }, globe_hidden
            page.mouse.click(*globe_hit["point"])
            page.wait_for_timeout(50)
            assert page.evaluate("document.getElementById('mapsvgModal')._olmap._cellInspectorElement.hidden")
            page.check("#mapMeshVisible")

            globe_control_lock = page.evaluate(
                """async () => { const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe,
                  zoomIn=map._globeContainer.querySelector('.maplibregl-ctrl-zoom-in');
                  globe.jumpTo({center:[0,0],zoom:1,bearing:0,pitch:0});const before=globe.getZoom(),disabledBefore=zoomIn.disabled;
                  setOlExportBusy(map,true);const disabledDuring=zoomIn.disabled;zoomIn.click();await new Promise(resolve=>setTimeout(resolve,50));
                  const during=globe.getZoom();setOlExportBusy(map,false);
                  return {before,during,disabledBefore,disabledDuring,disabledAfter:zoomIn.disabled}; }"""
            )
            assert globe_control_lock == {
                "before": 1,
                "during": 1,
                "disabledBefore": False,
                "disabledDuring": True,
                "disabledAfter": False,
            }, globe_control_lock

            page.locator("#mapExportMenu").evaluate("element => element.open=true")
            page.select_option("#mapExportScope", "view")
            page.select_option("#mapExportSize", "1920x1080")
            globe_view_export = page.evaluate(
                """async () => { const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe,
                  realCompose=composeGlobeCanvas,realPersist=persistMapPng;let captured=null;
                  globe.jumpTo({center:[18,12],zoom:2.4,bearing:13,pitch:18});
                  const state=()=>({bounds:globe.getBounds().toArray(),camera:[globe.getCenter().lng,globe.getCenter().lat,globe.getZoom(),globe.getBearing(),globe.getPitch()],
                    ratio:globe.getPixelRatio(),override:globe._overridePixelRatio??null,size:[globe.getCanvas().width,globe.getCanvas().height]});
                  const before=state();composeGlobeCanvas=(target,width,height,contain)=>{captured={...state(),contain};return realCompose(target,width,height,contain)};
                  persistMapPng=async()=>'';
                  try{ await saveGlobeMapPng(map); }finally{ composeGlobeCanvas=realCompose;persistMapPng=realPersist; }
                  return {before,captured,after:state(),sourceStable:globe.getSource('mesh')===window.__globeSource,setDataCalls:window.__globeSetDataCalls}; }"""
            )
            for key in ("bounds", "camera"):
                before_values = sum(globe_view_export["before"][key], []) if key == "bounds" else globe_view_export["before"][key]
                captured_values = sum(globe_view_export["captured"][key], []) if key == "bounds" else globe_view_export["captured"][key]
                after_values = sum(globe_view_export["after"][key], []) if key == "bounds" else globe_view_export["after"][key]
                assert all(abs(a - b) < 0.01 for a, b in zip(before_values, captured_values)), globe_view_export
                assert all(abs(a - b) < 0.01 for a, b in zip(before_values, after_values)), globe_view_export
            assert globe_view_export["captured"]["contain"] and globe_view_export["captured"]["size"][0] <= 1920 and globe_view_export["captured"]["size"][1] <= 1080, globe_view_export
            assert (1920 in globe_view_export["captured"]["size"] or 1080 in globe_view_export["captured"]["size"]) and globe_view_export["captured"]["ratio"] > globe_view_export["before"]["ratio"], globe_view_export
            assert globe_view_export["after"]["ratio"] == globe_view_export["before"]["ratio"], globe_view_export
            assert globe_view_export["before"]["override"] is None and globe_view_export["captured"]["override"] == globe_view_export["captured"]["ratio"] and globe_view_export["after"]["override"] is None, globe_view_export
            assert globe_view_export["sourceStable"] and globe_view_export["setDataCalls"] == 0, globe_view_export

            page.select_option("#mapExportScope", "global")
            page.select_option("#mapExportSize", "1920x1080")
            globe_export_before = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe;
                  globe.jumpTo({center:[25,5],zoom:1.75,bearing:17,pitch:22});
                  return {center:globe.getCenter().toArray(),zoom:globe.getZoom(),bearing:globe.getBearing(),pitch:globe.getPitch(),
                    size:[globe.getCanvas().getBoundingClientRect().width,globe.getCanvas().getBoundingClientRect().height],
                    controls:Array.from(document.querySelectorAll('#mapTools button,#mapTools select,#mapTools input')).map(control=>({id:control.id,disabled:control.disabled}))}; }"""
            )
            with page.expect_download(timeout=120_000) as download_info:
                page.evaluate("saveOlMapPng(document.getElementById('mapsvgModal')._olmap)")
            with tempfile.TemporaryDirectory() as tmpdir:
                png = Path(tmpdir) / download_info.value.suggested_filename
                download_info.value.save_as(png)
                globe_png_bytes = png.read_bytes()
                globe_png_size = list(struct.unpack(">II", globe_png_bytes[16:24]))
                assert globe_png_bytes[:8] == b"\x89PNG\r\n\x1a\n" and globe_png_bytes[12:16] == b"IHDR"
                assert globe_png_size == [1920, 1080] and len(globe_png_bytes) > 10_000, (globe_png_size, len(globe_png_bytes))
            globe_export_after = page.evaluate(
                """() => { const map=document.getElementById('mapsvgModal')._olmap,globe=map._globe,rect=globe.getCanvas().getBoundingClientRect();
                  return {active:map._globeActive,mapStable:globe===window.__globeMap,sourceStable:globe.getSource('mesh')===window.__globeSource,
                    rawStable:_meshGeojson===window.__globeRaw,setDataCalls:window.__globeSetDataCalls,
                    center:globe.getCenter().toArray(),zoom:globe.getZoom(),bearing:globe.getBearing(),pitch:globe.getPitch(),size:[rect.width,rect.height],
                    controls:Array.from(document.querySelectorAll('#mapTools button,#mapTools select,#mapTools input')).map(control=>({id:control.id,disabled:control.disabled}))}; }"""
            )
            assert globe_export_after["active"] and globe_export_after["mapStable"] and globe_export_after["sourceStable"], globe_export_after
            assert globe_export_after["rawStable"] and globe_export_after["setDataCalls"] == 0, globe_export_after
            for key in ("center", "size"):
                assert all(abs(a - b) < 0.01 for a, b in zip(globe_export_after[key], globe_export_before[key])), (globe_export_before, globe_export_after)
            for key in ("zoom", "bearing", "pitch"):
                assert abs(globe_export_after[key] - globe_export_before[key]) < 0.01, (globe_export_before, globe_export_after)
            assert globe_export_after["controls"] == globe_export_before["controls"], (globe_export_before, globe_export_after)

            browser.close()
    finally:
        server.shutdown()

    assert not errors, errors
    print(json.dumps({"menuBounds": menu_bounds, "antimeridian": dateline, "basemaps": basemap_results, "controls": control_state, "geographic": geographic, "utmNorth": utm_north, "utmSouth": utm_south, "watershedUtm": watershed_frame, "norwayUtm": norway_utm, "svalbardUtm": svalbard_utm, "closeEmpty": close_empty, "closeMeshUtm": close_mesh_utm, "watershedDateline": watershed_dateline, "emptyProjection": empty_projection, "inspectorReplacement": inspector_replacement, "resizes": resize_results, "bboxFit": bbox_fit, "worldView": world_view, "worldMargins": world_margins, "tooltipLeave": True, "meshHiddenHitTest": mesh_hidden_hit_test, "measure": measure_active, "smallRegion": small_region, "exportControlsDisabled": True, "regionPng": png_size, "globalPng": global_png_size, "globeInitial": globe_initial, "globeIdentity": globe_identity, "globeResizes": globe_resizes, "globeAntimeridian": globe_antimeridian, "globeInspector": globe_inspector, "globeHidden": globe_hidden, "globeControlLock": globe_control_lock, "globeViewExport": globe_view_export, "globePng": globe_png_size, "globeExportRestored": True}, indent=2))


if __name__ == "__main__":
    main()
