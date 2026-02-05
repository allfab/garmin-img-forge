#!/usr/bin/env python3
"""Create a test Shapefile with various fields for CreateField testing."""
from osgeo import ogr, osr

# Create spatial reference (WGS84)
srs = osr.SpatialReference()
srs.ImportFromEPSG(4326)

# Create driver
driver = ogr.GetDriverByName("ESRI Shapefile")

# Create POI shapefile
ds = driver.CreateDataSource("test_pois.shp")
layer = ds.CreateLayer("test_pois", srs, ogr.wkbPoint)

# Add fields - some match Polish Map schema, some don't
layer.CreateField(ogr.FieldDefn("Type", ogr.OFTString))
layer.CreateField(ogr.FieldDefn("Label", ogr.OFTString))
layer.CreateField(ogr.FieldDefn("EndLevel", ogr.OFTInteger))
layer.CreateField(ogr.FieldDefn("Levels", ogr.OFTString))
layer.CreateField(ogr.FieldDefn("ROAD_CLASS", ogr.OFTString))  # Unknown - should be ignored
layer.CreateField(ogr.FieldDefn("SPEED_KMH", ogr.OFTInteger))  # Unknown - should be ignored
layer.CreateField(ogr.FieldDefn("NOTES", ogr.OFTString))       # Unknown - should be ignored

# Create features
features = [
    {"Type": "0x2C00", "Label": "Restaurant Paris", "EndLevel": 3, "Levels": "0-3",
     "ROAD_CLASS": "A", "SPEED_KMH": 50, "NOTES": "Good food", "lon": 2.3522, "lat": 48.8566},
    {"Type": "0x2F00", "Label": "Hotel Lyon", "EndLevel": 2, "Levels": "0-2",
     "ROAD_CLASS": "B", "SPEED_KMH": 30, "NOTES": "4 stars", "lon": 4.8357, "lat": 45.7640},
    {"Type": "0x2E00", "Label": "Gas Station Marseille", "EndLevel": 4, "Levels": "0-4",
     "ROAD_CLASS": "C", "SPEED_KMH": 90, "NOTES": "24h", "lon": 5.3698, "lat": 43.2965},
]

for f_data in features:
    feature = ogr.Feature(layer.GetLayerDefn())
    feature.SetField("Type", f_data["Type"])
    feature.SetField("Label", f_data["Label"])
    feature.SetField("EndLevel", f_data["EndLevel"])
    feature.SetField("Levels", f_data["Levels"])
    feature.SetField("ROAD_CLASS", f_data["ROAD_CLASS"])
    feature.SetField("SPEED_KMH", f_data["SPEED_KMH"])
    feature.SetField("NOTES", f_data["NOTES"])
    
    point = ogr.Geometry(ogr.wkbPoint)
    point.AddPoint(f_data["lon"], f_data["lat"])
    feature.SetGeometry(point)
    
    layer.CreateFeature(feature)

ds = None
print("Created test_pois.shp with 3 POIs and 7 fields (4 Polish Map, 3 unknown)")
