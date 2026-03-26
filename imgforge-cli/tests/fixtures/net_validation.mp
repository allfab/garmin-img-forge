; Polish Map fixture: NET subfile validation
; Story 14.3 — routes variees pour tester NET1 flags, NET3 tri, TRE routing bit
;
; Routes:
;   1. Autoroute a peage (speed=7, class=4, toll, bidirectional)
;   2. Departementale bidirectionnelle (speed=5, class=2)
;   3. Sens unique (speed=3, class=1, oneway)
;   4. Rond-point segment (speed=2, class=1, oneway, roundabout)
;   5. Route avec restrictions acces (pedestrian+truck denied)
;
[IMG ID]
Name=NET Validation Test
ID=63240004
CodePage=1252
Levels=1
Level0=24
[END-IMG ID]

; Road 1: Autoroute A480 (toll, bidirectional, speed=7, class=4)
[POLYLINE]
Type=0x01
Label=A480
Data0=(45.190000,5.720000),(45.190000,5.740000)
EndLevel=3
RoadID=1
RouteParam=7,4,0,1,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 2: Departementale D1075 (bidirectional, speed=5, class=2)
[POLYLINE]
Type=0x02
Label=D1075
Data0=(45.180000,5.720000),(45.180000,5.740000)
EndLevel=3
RoadID=2
RouteParam=5,2,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 3: Sens unique (speed=3, class=1, oneway forward)
[POLYLINE]
Type=0x06
Label=Rue Victor Hugo
Data0=(45.185000,5.730000),(45.185000,5.740000)
EndLevel=2
RoadID=3
RouteParam=3,1,1,0,0,0,0,0,0,0,0,0
DirIndicator=1
Level=0
[END]

; Road 4: Rond-point (speed=2, class=1, oneway, roundabout)
[POLYLINE]
Type=0x0C
Label=Rond-point des Alpes
Data0=(45.180000,5.740000),(45.180100,5.740100),(45.180000,5.740200)
EndLevel=2
RoadID=4
RouteParam=2,1,1,0,0,0,0,0,0,0,0,0
DirIndicator=1
Roundabout=1
Level=0
[END]

; Road 5: Route avec restrictions (pedestrian + truck denied)
[POLYLINE]
Type=0x06
Label=Zone Industrielle
Data0=(45.175000,5.725000),(45.175000,5.735000)
EndLevel=2
RoadID=5
RouteParam=4,2,0,0,0,0,0,0,0,1,0,1
DirIndicator=0
Level=0
[END]

; Non-routable polyline (river, no routing attributes)
[POLYLINE]
Type=0x1A
Label=Isere
Data0=(45.190000,5.725000),(45.175000,5.725000)
EndLevel=3
[END]
