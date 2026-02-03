# Garmin Type Codes Reference

This document provides a reference for Garmin type codes used in Polish Map files. Type codes determine how features are displayed on Garmin GPS devices.

## Type Code Format

Type codes are hexadecimal values in the format:

```
Type=0xNNNN
```

Where `NNNN` is a 1-4 digit hexadecimal number. The `0x` prefix is optional in some tools.

## POI Type Codes (Points of Interest)

### Food & Drink (0x2C00-0x2CFF)

| Code | Description |
|------|-------------|
| 0x2C00 | Restaurant (Other) |
| 0x2C01 | Restaurant (American) |
| 0x2C02 | Restaurant (Asian) |
| 0x2C03 | Restaurant (Barbecue) |
| 0x2C04 | Restaurant (Chinese) |
| 0x2C05 | Restaurant (Deli/Bakery) |
| 0x2C06 | Restaurant (International) |
| 0x2C07 | Restaurant (Fast Food) |
| 0x2C08 | Restaurant (Italian) |
| 0x2C09 | Restaurant (Mexican) |
| 0x2C0A | Restaurant (Pizza) |
| 0x2C0B | Restaurant (Seafood) |
| 0x2C0C | Restaurant (Steak/Grill) |
| 0x2C0D | Restaurant (Bagel/Donut) |
| 0x2C0E | Restaurant (Cafe/Diner) |
| 0x2C0F | Restaurant (French) |
| 0x2C10 | Restaurant (German) |
| 0x2C11 | Restaurant (British Isles) |

### Lodging (0x2D00-0x2DFF)

| Code | Description |
|------|-------------|
| 0x2D00 | Lodging (Other) |
| 0x2D01 | Hotel/Motel |
| 0x2D02 | Bed & Breakfast/Inn |
| 0x2D03 | Campground/RV Park |
| 0x2D04 | Resort |
| 0x2D05 | Trailer Park |

### Shopping (0x2E00-0x2EFF)

| Code | Description |
|------|-------------|
| 0x2E00 | Shopping (Other) |
| 0x2E01 | Department Store |
| 0x2E02 | Grocery Store |
| 0x2E03 | General Merchandise |
| 0x2E04 | Shopping Center |
| 0x2E05 | Pharmacy |
| 0x2E06 | Convenience Store |
| 0x2E07 | Clothing Store |
| 0x2E08 | Home/Garden Store |
| 0x2E09 | Home Furnishings |
| 0x2E0A | Specialty Retail |
| 0x2E0B | Computer/Software |

### Auto Services (0x2F00-0x2FFF)

| Code | Description |
|------|-------------|
| 0x2F00 | Auto Service (Other) |
| 0x2F01 | Gas Station |
| 0x2F02 | Auto Rental |
| 0x2F03 | Auto Repair |
| 0x2F04 | Airport |
| 0x2F05 | Post Office |
| 0x2F06 | Bank/ATM |
| 0x2F07 | Auto Dealer |
| 0x2F08 | Ground Transportation |
| 0x2F09 | Marina |
| 0x2F0A | Wrecker Service |
| 0x2F0B | Parking |
| 0x2F0C | Rest Area/Tourist Info |
| 0x2F0D | Auto Club |
| 0x2F0E | Truck Stop |
| 0x2F0F | Rail Station |
| 0x2F10 | Transit Service |
| 0x2F11 | Ferry Terminal |
| 0x2F12 | Emergency/Government |
| 0x2F13 | Scales |
| 0x2F14 | Toll Booth |
| 0x2F15 | Bridge |
| 0x2F16 | Building |
| 0x2F17 | Tunnel |

### Attractions (0x2A00-0x2BFF)

| Code | Description |
|------|-------------|
| 0x2A00 | Attraction (Other) |
| 0x2A01 | Amusement/Theme Park |
| 0x2A02 | Museum/Historical |
| 0x2A03 | Library |
| 0x2A04 | Landmark |
| 0x2A05 | School |
| 0x2A06 | Park/Garden |
| 0x2A07 | Zoo/Aquarium |
| 0x2A08 | Arena/Track |
| 0x2A09 | Hall/Auditorium |
| 0x2A0A | Winery |
| 0x2A0B | Place of Worship |
| 0x2A0C | Hot Spring |

### Entertainment (0x2B00-0x2BFF)

| Code | Description |
|------|-------------|
| 0x2B00 | Entertainment (Other) |
| 0x2B01 | Live Theater |
| 0x2B02 | Bar/Nightclub |
| 0x2B03 | Movie Theater |
| 0x2B04 | Casino |
| 0x2B05 | Golf Course |
| 0x2B06 | Ski Resort |
| 0x2B07 | Bowling |
| 0x2B08 | Ice Skating |
| 0x2B09 | Swimming Pool |
| 0x2B0A | Sports/Fitness |
| 0x2B0B | Sports Activity |

### Medical/Community (0x3000-0x30FF)

| Code | Description |
|------|-------------|
| 0x3000 | Medical (Other) |
| 0x3001 | Hospital |
| 0x3002 | Doctor/Medical Service |
| 0x3003 | Dentist |
| 0x3004 | Veterinarian |
| 0x3005 | Community Service |
| 0x3006 | Government Office |
| 0x3007 | City Hall |
| 0x3008 | Court House |

### Emergency Services

| Code | Description |
|------|-------------|
| 0x4000 | Police Station |
| 0x4100 | Fire Department |
| 0x4200 | Emergency Room |

### Geographic Features (0x6400-0x66FF)

| Code | Description |
|------|-------------|
| 0x6400 | Summit |
| 0x6401 | Locale |
| 0x6402 | Bench Mark |
| 0x6403 | Bridge |
| 0x6404 | Building |
| 0x6405 | Cemetery |
| 0x6406 | Church |
| 0x6407 | Civil |
| 0x6408 | Crossing |
| 0x6409 | Dam |
| 0x640A | Flat |
| 0x640B | Forest |
| 0x640C | Gap |
| 0x640D | Gut |
| 0x640E | Harbor |
| 0x640F | Hospital |
| 0x6410 | Island |
| 0x6411 | Lake |
| 0x6412 | Locale |
| 0x6413 | Park |
| 0x6414 | Pillar |
| 0x6415 | Post Office |
| 0x6416 | Populated Place |

## Polyline Type Codes (Roads and Paths)

### Roads (0x0001-0x000F)

| Code | Description | Usage |
|------|-------------|-------|
| 0x0001 | Major Highway | Interstate, Motorway |
| 0x0002 | Principal Highway | US/State Highway |
| 0x0003 | Other Highway | Regional Highway |
| 0x0004 | Arterial Road | Major Urban Road |
| 0x0005 | Collector Road | Secondary Urban |
| 0x0006 | Residential Street | Local Roads |
| 0x0007 | Alley/Private Road | Restricted Access |
| 0x0008 | Highway Ramp | On/Off Ramps |
| 0x0009 | Highway Connector | Interchanges |
| 0x000A | Unpaved Road | Gravel, Dirt |
| 0x000B | Major Connector | Highway Links |
| 0x000C | Roundabout | Traffic Circle |
| 0x000D | Proposed Road | Future Construction |
| 0x000E | 4WD Road | Off-road Only |

### Trails and Paths (0x0010-0x001F)

| Code | Description |
|------|-------------|
| 0x0010 | Unpaved Trail |
| 0x0014 | Railroad |
| 0x0015 | Shoreline |
| 0x0016 | Trail |
| 0x0017 | Boundary (Int'l) |
| 0x0018 | Stream |
| 0x0019 | Intermittent Stream |
| 0x001A | River/Canal |
| 0x001B | Boundary (State) |
| 0x001C | Boundary (County) |
| 0x001D | Power Line |
| 0x001E | Pipeline |
| 0x001F | Ferry |

### Water Features (0x0020-0x002F)

| Code | Description |
|------|-------------|
| 0x0020 | Perennial Stream |
| 0x0021 | Intermittent Stream |
| 0x0022 | River |
| 0x0023 | Canal/Aqueduct |
| 0x0024 | Ditch/Drain |
| 0x0025 | Lake Shoreline |
| 0x0026 | Coastline |

### Boundaries (0x0030-0x003F)

| Code | Description |
|------|-------------|
| 0x0030 | Boundary (Other) |
| 0x0031 | National Park |
| 0x0032 | County/Parish |
| 0x0033 | City/Town |
| 0x0034 | Military |
| 0x0035 | Indian Reservation |
| 0x0036 | Miscellaneous |

## Polygon Type Codes (Areas)

### Urban Areas (0x0001-0x000F)

| Code | Description |
|------|-------------|
| 0x0001 | Large City (>200K) |
| 0x0002 | Small City (10K-200K) |
| 0x0003 | Town (<10K) |
| 0x0004 | Military Base |
| 0x0005 | Parking Lot |
| 0x0006 | Parking Garage |
| 0x0007 | Airport |
| 0x0008 | Shopping Center |
| 0x0009 | Marina |
| 0x000A | University |
| 0x000B | Hospital |
| 0x000C | Industrial |
| 0x000D | Reservation |
| 0x000E | Airport Runway |

### Parks and Recreation (0x0010-0x001F)

| Code | Description |
|------|-------------|
| 0x0010 | State Park |
| 0x0011 | National Park |
| 0x0012 | City Park |
| 0x0013 | Golf Course |
| 0x0014 | Sports Field |
| 0x0015 | Cemetery |
| 0x0016 | Generic Park |
| 0x0017 | National Forest |
| 0x0018 | City Square |
| 0x0019 | Generic Manmade |

### Natural Features (0x0020-0x004F)

| Code | Description |
|------|-------------|
| 0x0020 | Land (Generic) |
| 0x0028 | Ocean |
| 0x0029 | Blue (Unknown) |
| 0x003C | Lake/Pond |
| 0x003D | Lake/Pond |
| 0x003E | Lake |
| 0x003F | Lake |
| 0x0040 | Lake |
| 0x0041 | Lake |
| 0x0042 | Major Lake |
| 0x0043 | Lake |
| 0x0044 | Lake (Large) |
| 0x0045 | Blue (Unknown) |
| 0x0046 | River |
| 0x0047 | River (Large) |
| 0x0048 | River (Small) |
| 0x0049 | Swamp/Marsh |
| 0x004C | Forest |
| 0x004D | Scrub |
| 0x004E | Wetland |
| 0x004F | Tundra |

### Land Cover (0x0050-0x006F)

| Code | Description |
|------|-------------|
| 0x0050 | Flat |
| 0x0051 | Sand |
| 0x0052 | Gravel |
| 0x0053 | Glacier |
| 0x0054 | Orchard |
| 0x0055 | Vineyard |
| 0x0056 | Crop Land |

## Usage Examples

### Restaurant POI

```
[POI]
Type=0x2C00
Label=Le Bon Restaurant
Data0=(48.8566,2.3522)
EndLevel=3
[END]
```

### Highway Polyline

```
[POLYLINE]
Type=0x0001
Label=A1 Autoroute
Data0=(48.9,2.3),(49.0,2.4),(49.1,2.5)
EndLevel=5
[END]
```

### Forest Polygon

```
[POLYGON]
Type=0x004C
Label=Forêt de Fontainebleau
Data0=(48.40,2.60),(48.45,2.65),(48.40,2.70),(48.35,2.65),(48.40,2.60)
EndLevel=3
[END]
```

## Custom Type Codes

Type codes in the range 0x10000-0x1FFFF can be used for custom POI types. These require a custom TYP file to define the symbology.

## References

- [cGPSmapper Type Codes](http://www.cgpsmapper.com/manual/chapter5.htm)
- [OSM to Garmin POI Types](https://wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/POI_Types)
- [Garmin IMG File Format](https://wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin)
