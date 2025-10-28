import xarray
import mpas_tools
from mpas_tools.mesh.creation.jigsaw_to_netcdf import jigsaw_to_netcdf
from mpas_tools.mesh.conversion import convert
from mpas_tools.io import write_netcdf
from mpas_tools.ocean.inject_meshDensity import inject_spherical_meshDensity


#res1 = 240.
#out_dir='grid_'+str(int(res1))+'km'
#out_base='grid_'+str(int(res1))+'km'

out_dir='grid_30-120km'
out_base='grid_30-120km'
out_basepath=out_dir+"/"+out_base
out_filename=out_dir+"/"+out_base+"_mpas_wei5.nc"

#convert to mpas grid specific format
print ("Convert to mpas grid specific format")
write_netcdf(
        convert(xarray.open_dataset('/Users/zhongwangwei/Desktop/untitled folder/EarthMesh_251027/cases/ATMOS_hex_N64_refine2_global_Simple_LOM67_251027/result/MPASOUT_NXP0064_global_Simple.nc4'), 
        dir=out_dir,
        graphInfoFileName=out_basepath+"_graph.info"),
        out_filename)
