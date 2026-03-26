; Polish Map fixture: realistic road network for graph builder testing
; Story 14.2 — ~10 polylines: intersections, one-way, roundabout, bridge
; Network layout (Grenoble area, approximate):
;
;   A(45.19,5.72) ---- B(45.19,5.73) ---- C(45.19,5.74)
;                       |                   |
;                       |                   |
;   D(45.18,5.72) ---- E(45.18,5.73) ---- F(45.18,5.74)
;                       |
;                       G(45.17,5.73) (one-way E→G)
;
;   Bridge over E: H(45.185,5.73)→I(45.175,5.73) at Level=1
;   Roundabout at F: small loop F→F'→F (one-way)
;
[IMG ID]
Name=Routing Graph Test
ID=63240003
CodePage=1252
Levels=2
Level0=24
Level1=18
[END-IMG ID]

; Road 1: A→B (bidirectional, departementale)
[POLYLINE]
Type=0x02
Label=D1012
Data0=(45.190000,5.720000),(45.190000,5.730000)
EndLevel=3
RoadID=1
RouteParam=5,2,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 2: B→C (bidirectional, departementale)
[POLYLINE]
Type=0x02
Label=D1012
Data0=(45.190000,5.730000),(45.190000,5.740000)
EndLevel=3
RoadID=2
RouteParam=5,2,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 3: D→E (bidirectional, communale)
[POLYLINE]
Type=0x06
Label=Rue de la Gare
Data0=(45.180000,5.720000),(45.180000,5.730000)
EndLevel=2
RoadID=3
RouteParam=3,1,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 4: E→F (bidirectional, communale)
[POLYLINE]
Type=0x06
Label=Rue Victor Hugo
Data0=(45.180000,5.730000),(45.180000,5.740000)
EndLevel=2
RoadID=4
RouteParam=3,1,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 5: B→E (bidirectional, communale, north-south link)
[POLYLINE]
Type=0x06
Label=Avenue Jean Jaures
Data0=(45.190000,5.730000),(45.180000,5.730000)
EndLevel=2
RoadID=5
RouteParam=3,1,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 6: C→F (bidirectional, communale, north-south link)
[POLYLINE]
Type=0x06
Label=Rue Pasteur
Data0=(45.190000,5.740000),(45.180000,5.740000)
EndLevel=2
RoadID=6
RouteParam=3,1,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 7: E→G (one-way forward, residential)
[POLYLINE]
Type=0x07
Label=Impasse du Lac
Data0=(45.180000,5.730000),(45.170000,5.730000)
EndLevel=2
RoadID=7
RouteParam=2,0,1,0,0,0,0,0,0,0,0,0
DirIndicator=1
Level=0
[END]

; Road 8: Bridge H→I over node E (Level=1, bidirectional)
[POLYLINE]
Type=0x01
Label=Pont de la Liberte
Data0=(45.185000,5.730000),(45.175000,5.730000)
EndLevel=3
RoadID=8
RouteParam=5,2,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=1
[END]

; Road 9: Roundabout segment (one-way forward, at F)
[POLYLINE]
Type=0x0C
Label=Rond-point des Alpes
Data0=(45.180000,5.740000),(45.180100,5.740100),(45.180000,5.740200)
EndLevel=2
RoadID=9
RouteParam=2,1,1,0,0,0,0,0,0,0,0,0
DirIndicator=1
Roundabout=1
Level=0
[END]

; Road 10: A→D (bidirectional, communale, west side link)
[POLYLINE]
Type=0x06
Label=Rue des Ecoles
Data0=(45.190000,5.720000),(45.180000,5.720000)
EndLevel=2
RoadID=10
RouteParam=3,1,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Non-routable polyline (river, no routing attributes)
[POLYLINE]
Type=0x1A
Label=Isere
Data0=(45.190000,5.725000),(45.180000,5.725000),(45.170000,5.725000)
EndLevel=3
[END]
