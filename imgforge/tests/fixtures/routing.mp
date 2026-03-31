; Fixture: routing.mp
; Tests: routable polylines with RoadID + RouteParam
[IMG ID]
ID=99990002
Name=Routing Test Map
Levels=24,20
CodePage=1252
[END-IMG ID]
[POLYLINE]
Type=0x06
Label=Autoroute A35
RoadID=1001
RouteParam=7,4,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Data0=(48.5700,7.7400),(48.5800,7.7500),(48.5900,7.7600)
[END]
[POLYLINE]
Type=0x02
Label=Route Departementale
RoadID=1002
RouteParam=5,3,0,0,0,0,0,0,0,0,0,0
Data0=(48.5800,7.7500),(48.5850,7.7550),(48.5900,7.7650)
[END]
[POLYLINE]
Type=0x04
Label=Rue du Village
RoadID=1003
RouteParam=3,0,0,0,0,0,0,0,0,0,0,0
Data0=(48.5850,7.7550),(48.5860,7.7560)
[END]
[POI]
Type=0x2A00
Label=Station Service
Data0=(48.5800,7.7500)
[END]
