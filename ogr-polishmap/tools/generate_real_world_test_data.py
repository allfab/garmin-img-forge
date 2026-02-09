#!/usr/bin/env python3
"""
Generate synthetic test data for Story 4.3: Real-World SIG Integration Tests.

Creates representative test datasets for BDTOPO, OSM, and generic use cases.
All data is synthetic but representative of real-world structures.

Usage: python3 tools/generate_real_world_test_data.py
"""

import os
import json
from osgeo import ogr, osr

# Base directory for real_world test data
BASE_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                        "test", "data", "real_world")


def create_srs_wgs84():
    """Create WGS84 spatial reference."""
    srs = osr.SpatialReference()
    srs.ImportFromEPSG(4326)
    return srs


# ============================================================
# Task 1.2: BDTOPO COMMUNE_sample.shp (3 communes La Réunion)
# ============================================================

def create_bdtopo_commune():
    """Create synthetic BDTOPO COMMUNE sample with 3 communes from La Réunion.

    MultiPolygon geometry to test Story 4.2 decomposition.
    Fields match real BDTOPO schema: NAME, MP_TYPE, Country, RegionName, CityName, Zip, EndLevel, MPBITLEVEL.
    """
    out_path = os.path.join(BASE_DIR, "bdtopo", "COMMUNE_sample.shp")
    driver = ogr.GetDriverByName("ESRI Shapefile")

    if os.path.exists(out_path):
        driver.DeleteDataSource(out_path)

    ds = driver.CreateDataSource(out_path)
    srs = create_srs_wgs84()
    layer = ds.CreateLayer("COMMUNE", srs, ogr.wkbMultiPolygon)

    # BDTOPO-like fields
    layer.CreateField(ogr.FieldDefn("NAME", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("MP_TYPE", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("Country", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("RegionName", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("CityName", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("Zip", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("EndLevel", ogr.OFTInteger))
    layer.CreateField(ogr.FieldDefn("MPBITLEVEL", ogr.OFTString))

    # Commune 1: Les Avirons (MultiPolygon with 2 parts)
    feat = ogr.Feature(layer.GetLayerDefn())
    feat.SetField("NAME", "Les Avirons")
    feat.SetField("MP_TYPE", "0x54")
    feat.SetField("Country", "France~[0x1d]FRA")
    feat.SetField("RegionName", "La Réunion")
    feat.SetField("CityName", "Les Avirons")
    feat.SetField("Zip", "97425")
    feat.SetField("EndLevel", 3)
    feat.SetField("MPBITLEVEL", "17")

    mp = ogr.Geometry(ogr.wkbMultiPolygon)
    # Part 1
    poly1 = ogr.Geometry(ogr.wkbPolygon)
    ring1 = ogr.Geometry(ogr.wkbLinearRing)
    ring1.AddPoint(55.3200, -21.2400)
    ring1.AddPoint(55.3300, -21.2400)
    ring1.AddPoint(55.3300, -21.2300)
    ring1.AddPoint(55.3200, -21.2300)
    ring1.AddPoint(55.3200, -21.2400)
    poly1.AddGeometry(ring1)
    mp.AddGeometry(poly1)
    # Part 2
    poly2 = ogr.Geometry(ogr.wkbPolygon)
    ring2 = ogr.Geometry(ogr.wkbLinearRing)
    ring2.AddPoint(55.3350, -21.2450)
    ring2.AddPoint(55.3450, -21.2450)
    ring2.AddPoint(55.3450, -21.2350)
    ring2.AddPoint(55.3350, -21.2350)
    ring2.AddPoint(55.3350, -21.2450)
    poly2.AddGeometry(ring2)
    mp.AddGeometry(poly2)

    feat.SetGeometry(mp)
    layer.CreateFeature(feat)
    feat = None

    # Commune 2: Saint-Pierre (single polygon in MultiPolygon)
    feat = ogr.Feature(layer.GetLayerDefn())
    feat.SetField("NAME", "Saint-Pierre")
    feat.SetField("MP_TYPE", "0x54")
    feat.SetField("Country", "France~[0x1d]FRA")
    feat.SetField("RegionName", "La Réunion")
    feat.SetField("CityName", "Saint-Pierre")
    feat.SetField("Zip", "97410")
    feat.SetField("EndLevel", 3)
    feat.SetField("MPBITLEVEL", "17")

    mp = ogr.Geometry(ogr.wkbMultiPolygon)
    poly = ogr.Geometry(ogr.wkbPolygon)
    ring = ogr.Geometry(ogr.wkbLinearRing)
    ring.AddPoint(55.4700, -21.3400)
    ring.AddPoint(55.4900, -21.3400)
    ring.AddPoint(55.4900, -21.3200)
    ring.AddPoint(55.4700, -21.3200)
    ring.AddPoint(55.4700, -21.3400)
    poly.AddGeometry(ring)
    mp.AddGeometry(poly)
    feat.SetGeometry(mp)
    layer.CreateFeature(feat)
    feat = None

    # Commune 3: Le Tampon (MultiPolygon with 3 parts)
    feat = ogr.Feature(layer.GetLayerDefn())
    feat.SetField("NAME", "Le Tampon")
    feat.SetField("MP_TYPE", "0x54")
    feat.SetField("Country", "France~[0x1d]FRA")
    feat.SetField("RegionName", "La Réunion")
    feat.SetField("CityName", "Le Tampon")
    feat.SetField("Zip", "97430")
    feat.SetField("EndLevel", 3)
    feat.SetField("MPBITLEVEL", "17")

    mp = ogr.Geometry(ogr.wkbMultiPolygon)
    for i in range(3):
        poly = ogr.Geometry(ogr.wkbPolygon)
        ring = ogr.Geometry(ogr.wkbLinearRing)
        base_lon = 55.5000 + i * 0.02
        base_lat = -21.2800
        ring.AddPoint(base_lon, base_lat)
        ring.AddPoint(base_lon + 0.01, base_lat)
        ring.AddPoint(base_lon + 0.01, base_lat + 0.01)
        ring.AddPoint(base_lon, base_lat + 0.01)
        ring.AddPoint(base_lon, base_lat)
        poly.AddGeometry(ring)
        mp.AddGeometry(poly)
    feat.SetGeometry(mp)
    layer.CreateFeature(feat)
    feat = None

    ds = None
    print(f"  Created: {out_path} (3 communes, MultiPolygon)")


# ============================================================
# Task 1.3: BDTOPO ROUTE_sample.shp (10 routes)
# ============================================================

def create_bdtopo_route():
    """Create synthetic BDTOPO ROUTE sample with 10 routes."""
    out_path = os.path.join(BASE_DIR, "bdtopo", "ROUTE_sample.shp")
    driver = ogr.GetDriverByName("ESRI Shapefile")

    if os.path.exists(out_path):
        driver.DeleteDataSource(out_path)

    ds = driver.CreateDataSource(out_path)
    srs = create_srs_wgs84()
    layer = ds.CreateLayer("ROUTE", srs, ogr.wkbLineString)

    layer.CreateField(ogr.FieldDefn("NAME", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("MP_TYPE", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("RoadID", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("EndLevel", ogr.OFTInteger))

    routes = [
        ("Route Nationale 1", "0x02", "RN1", 4),
        ("Route Nationale 2", "0x02", "RN2", 4),
        ("Route Nationale 3", "0x02", "RN3", 4),
        ("Route Départementale 11", "0x04", "RD11", 3),
        ("Route Départementale 12", "0x04", "RD12", 3),
        ("Route Départementale 26", "0x04", "RD26", 3),
        ("Rue du Commerce", "0x06", "RC001", 2),
        ("Avenue des Mascareignes", "0x06", "AM001", 2),
        ("Chemin Rural 5", "0x0A", "CR5", 1),
        ("Sentier du Volcan", "0x16", "SV001", 1),
    ]

    for i, (name, mp_type, road_id, end_level) in enumerate(routes):
        feat = ogr.Feature(layer.GetLayerDefn())
        feat.SetField("NAME", name)
        feat.SetField("MP_TYPE", mp_type)
        feat.SetField("RoadID", road_id)
        feat.SetField("EndLevel", end_level)

        line = ogr.Geometry(ogr.wkbLineString)
        base_lon = 55.4500 + i * 0.005
        base_lat = -21.3000
        for j in range(5):
            line.AddPoint(base_lon + j * 0.002, base_lat + j * 0.001)
        feat.SetGeometry(line)
        layer.CreateFeature(feat)
        feat = None

    ds = None
    print(f"  Created: {out_path} (10 routes, LineString)")


# ============================================================
# Task 1.4: OSM roads GeoJSON (10 features, LineString + MultiLineString)
# ============================================================

def create_osm_roads():
    """Create synthetic OSM roads GeoJSON with 10 features (LineString + MultiLineString)."""
    out_path = os.path.join(BASE_DIR, "osm", "roads.geojson")

    features = []

    # 7 simple LineString roads
    simple_roads = [
        ("Rue de la Paix", "residential", ""),
        ("Avenue Victor Hugo", "secondary", "D42"),
        ("Boulevard Gambetta", "primary", "N7"),
        ("Rue des Fleurs", "residential", ""),
        ("Route de Lyon", "trunk", "A43"),
        ("Chemin des Vignes", "track", ""),
        ("Allée des Platanes", "living_street", ""),
    ]

    for i, (name, highway, ref) in enumerate(simple_roads):
        coords = []
        base_lon = 2.3000 + i * 0.005
        base_lat = 48.8500
        for j in range(4):
            coords.append([base_lon + j * 0.001, base_lat + j * 0.0005])

        feature = {
            "type": "Feature",
            "properties": {
                "name": name,
                "highway": highway,
                "ref": ref if ref else None,
            },
            "geometry": {
                "type": "LineString",
                "coordinates": coords,
            }
        }
        features.append(feature)

    # 3 MultiLineString roads (fragmented roads)
    multi_roads = [
        ("Route Nationale 20", "primary", "N20"),
        ("Autoroute du Soleil", "motorway", "A6"),
        ("Périphérique", "motorway", "BP"),
    ]

    for i, (name, highway, ref) in enumerate(multi_roads):
        parts = []
        for p in range(3):
            coords = []
            base_lon = 2.3500 + i * 0.01 + p * 0.003
            base_lat = 48.8600 + p * 0.002
            for j in range(3):
                coords.append([base_lon + j * 0.001, base_lat + j * 0.0005])
            parts.append(coords)

        feature = {
            "type": "Feature",
            "properties": {
                "name": name,
                "highway": highway,
                "ref": ref,
            },
            "geometry": {
                "type": "MultiLineString",
                "coordinates": parts,
            }
        }
        features.append(feature)

    geojson = {
        "type": "FeatureCollection",
        "features": features,
    }

    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(geojson, f, indent=2, ensure_ascii=False)

    print(f"  Created: {out_path} (10 features: 7 LineString + 3 MultiLineString)")


# ============================================================
# Task 1.5: OSM POIs GeoJSON (20 features, Point)
# ============================================================

def create_osm_pois():
    """Create synthetic OSM POIs GeoJSON with 20 features."""
    out_path = os.path.join(BASE_DIR, "osm", "pois.geojson")

    pois = [
        ("Boulangerie du Coin", "bakery", "shop"),
        ("Pharmacie Centrale", "pharmacy", "amenity"),
        ("École Primaire Jules Ferry", "school", "amenity"),
        ("Mairie de Saint-Denis", "townhall", "amenity"),
        ("Gare de Lyon", "station", "railway"),
        ("Hôpital Nord", "hospital", "amenity"),
        ("Musée des Arts", "museum", "tourism"),
        ("Parc des Expositions", "exhibition_centre", "amenity"),
        ("Restaurant Le Provence", "restaurant", "amenity"),
        ("Hôtel de la Plage", "hotel", "tourism"),
        ("Bibliothèque Municipale", "library", "amenity"),
        ("Stade Olympique", "stadium", "leisure"),
        ("Cinéma Le Rex", "cinema", "amenity"),
        ("Poste Centrale", "post_office", "amenity"),
        ("Banque Populaire", "bank", "amenity"),
        ("Supermarché Casino", "supermarket", "shop"),
        ("Église Saint-Jacques", "place_of_worship", "amenity"),
        ("Jardin Botanique", "garden", "leisure"),
        ("Aéroport Roland Garros", "aerodrome", "aeroway"),
        ("Port de Commerce", "port", "harbour"),
    ]

    features = []
    for i, (name, poi_type, category) in enumerate(pois):
        lon = 2.3000 + (i % 5) * 0.005 + (i // 5) * 0.001
        lat = 48.8500 + (i % 5) * 0.002 + (i // 5) * 0.001

        feature = {
            "type": "Feature",
            "properties": {
                "name": name,
                category: poi_type,
            },
            "geometry": {
                "type": "Point",
                "coordinates": [lon, lat],
            }
        }
        features.append(feature)

    geojson = {
        "type": "FeatureCollection",
        "features": features,
    }

    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(geojson, f, indent=2, ensure_ascii=False)

    print(f"  Created: {out_path} (20 POI features)")


# ============================================================
# Task 1.6: Encoding test Shapefile (accented names FR/ES/DE)
# ============================================================

def create_encoding_test():
    """Create Shapefile with CP1252-compatible and edge-case characters."""
    out_path = os.path.join(BASE_DIR, "generic", "encoding_test.shp")
    driver = ogr.GetDriverByName("ESRI Shapefile")

    if os.path.exists(out_path):
        driver.DeleteDataSource(out_path)

    ds = driver.CreateDataSource(out_path)
    srs = create_srs_wgs84()
    layer = ds.CreateLayer("encoding_test", srs, ogr.wkbPoint,
                           options=["ENCODING=UTF-8"])

    layer.CreateField(ogr.FieldDefn("NAME", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("MP_TYPE", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("Country", ogr.OFTString))

    # Test data from AC5 specification
    test_names = [
        # French accents (CP1252 compatible)
        ("Château-Thierry", "0x2C00", "France"),
        ("Île-de-France", "0x2C00", "France"),
        ("Béziers", "0x2C00", "France"),
        ("Père-Lachaise", "0x2C00", "France"),
        ("Forêt de Fontainebleau", "0x2C00", "France"),
        ("Français à Paris", "0x2C00", "France"),
        # Spanish (CP1252 compatible)
        ("Peñíscola", "0x2C00", "España"),
        # German (CP1252 compatible)
        ("München", "0x2C00", "Deutschland"),
        ("Köln", "0x2C00", "Deutschland"),
        ("Düsseldorf", "0x2C00", "Deutschland"),
    ]

    for i, (name, mp_type, country) in enumerate(test_names):
        feat = ogr.Feature(layer.GetLayerDefn())
        feat.SetField("NAME", name)
        feat.SetField("MP_TYPE", mp_type)
        feat.SetField("Country", country)

        pt = ogr.Geometry(ogr.wkbPoint)
        pt.AddPoint(2.3000 + i * 0.01, 48.8500 + i * 0.005)
        feat.SetGeometry(pt)
        layer.CreateFeature(feat)
        feat = None

    ds = None
    print(f"  Created: {out_path} (10 features with accented names)")


# ============================================================
# Task 1.7: Large MultiPolygon Shapefile (100 parts)
# ============================================================

def create_large_multipolygon():
    """Create Shapefile with a single feature having a 100-part MultiPolygon."""
    out_path = os.path.join(BASE_DIR, "generic", "large_multipolygon.shp")
    driver = ogr.GetDriverByName("ESRI Shapefile")

    if os.path.exists(out_path):
        driver.DeleteDataSource(out_path)

    ds = driver.CreateDataSource(out_path)
    srs = create_srs_wgs84()
    layer = ds.CreateLayer("large_multipolygon", srs, ogr.wkbMultiPolygon)

    layer.CreateField(ogr.FieldDefn("NAME", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("MP_TYPE", ogr.OFTString))

    feat = ogr.Feature(layer.GetLayerDefn())
    feat.SetField("NAME", "Large Archipelago")
    feat.SetField("MP_TYPE", "0x4C")

    mp = ogr.Geometry(ogr.wkbMultiPolygon)
    for i in range(100):
        poly = ogr.Geometry(ogr.wkbPolygon)
        ring = ogr.Geometry(ogr.wkbLinearRing)
        # Grid layout: 10x10
        row = i // 10
        col = i % 10
        base_lon = 2.0 + col * 0.005
        base_lat = 48.0 + row * 0.005
        size = 0.003
        ring.AddPoint(base_lon, base_lat)
        ring.AddPoint(base_lon + size, base_lat)
        ring.AddPoint(base_lon + size, base_lat + size)
        ring.AddPoint(base_lon, base_lat + size)
        ring.AddPoint(base_lon, base_lat)
        poly.AddGeometry(ring)
        mp.AddGeometry(poly)

    feat.SetGeometry(mp)
    layer.CreateFeature(feat)
    feat = None

    ds = None
    print(f"  Created: {out_path} (1 feature, 100-part MultiPolygon)")


# ============================================================
# Task 1.8: Mixed geometries Shapefile (Points + Lines + Polygons)
# ============================================================

def create_mixed_geometries():
    """Create Shapefile with mixed geometry types for round-trip testing.

    Note: Shapefile only supports one geometry type per layer, so we create
    separate layers in 3 separate files to be combined.
    Actually for this test, we create a single Shapefile with Polygon type
    since the round-trip test (AC1) focuses on a single type.
    """
    out_path = os.path.join(BASE_DIR, "generic", "mixed_geometries.shp")
    driver = ogr.GetDriverByName("ESRI Shapefile")

    if os.path.exists(out_path):
        driver.DeleteDataSource(out_path)

    ds = driver.CreateDataSource(out_path)
    srs = create_srs_wgs84()
    layer = ds.CreateLayer("mixed_geometries", srs, ogr.wkbPolygon)

    layer.CreateField(ogr.FieldDefn("NAME", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("MP_TYPE", ogr.OFTString))
    layer.CreateField(ogr.FieldDefn("EndLevel", ogr.OFTInteger))

    polygons = [
        ("Zone Industrielle", "0x0E", 2),
        ("Parc National", "0x4C", 4),
        ("Lac", "0x3C", 3),
        ("Aéroport", "0x07", 4),
        ("Zone Militaire", "0x04", 2),
    ]

    for i, (name, mp_type, end_level) in enumerate(polygons):
        feat = ogr.Feature(layer.GetLayerDefn())
        feat.SetField("NAME", name)
        feat.SetField("MP_TYPE", mp_type)
        feat.SetField("EndLevel", end_level)

        poly = ogr.Geometry(ogr.wkbPolygon)
        ring = ogr.Geometry(ogr.wkbLinearRing)
        base_lon = 2.3000 + i * 0.02
        base_lat = 48.8000
        ring.AddPoint(base_lon, base_lat)
        ring.AddPoint(base_lon + 0.015, base_lat)
        ring.AddPoint(base_lon + 0.015, base_lat + 0.012)
        ring.AddPoint(base_lon, base_lat + 0.012)
        ring.AddPoint(base_lon, base_lat)
        poly.AddGeometry(ring)
        feat.SetGeometry(poly)
        layer.CreateFeature(feat)
        feat = None

    ds = None
    print(f"  Created: {out_path} (5 polygon features)")


# ============================================================
# Main
# ============================================================

def main():
    print("=" * 60)
    print("  Generating Real-World Test Data (Story 4.3)")
    print("=" * 60)
    print()

    os.makedirs(os.path.join(BASE_DIR, "bdtopo"), exist_ok=True)
    os.makedirs(os.path.join(BASE_DIR, "osm"), exist_ok=True)
    os.makedirs(os.path.join(BASE_DIR, "generic"), exist_ok=True)
    os.makedirs(os.path.join(BASE_DIR, "expected_outputs"), exist_ok=True)

    print("[Task 1.2] BDTOPO COMMUNE sample:")
    create_bdtopo_commune()

    print("[Task 1.3] BDTOPO ROUTE sample:")
    create_bdtopo_route()

    print("[Task 1.4] OSM roads GeoJSON:")
    create_osm_roads()

    print("[Task 1.5] OSM POIs GeoJSON:")
    create_osm_pois()

    print("[Task 1.6] Encoding test Shapefile:")
    create_encoding_test()

    print("[Task 1.7] Large MultiPolygon Shapefile:")
    create_large_multipolygon()

    print("[Task 1.8] Mixed geometries Shapefile:")
    create_mixed_geometries()

    print()
    print("All test data generated successfully!")

    # Print size summary
    total_size = 0
    for root, dirs, files in os.walk(BASE_DIR):
        for f in files:
            total_size += os.path.getsize(os.path.join(root, f))
    print(f"Total corpus size: {total_size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
