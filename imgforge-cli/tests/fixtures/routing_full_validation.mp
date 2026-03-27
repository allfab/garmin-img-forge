; Polish Map fixture: Validation réseau routier complet
; Story 14.5 — réseau type Grenoble pour valider le routage (AC1-AC4)
;
; Routes Level=0 (5 routes routables) :
;   A480_N      : autoroute nord  (speed=7, class=4, toll=true, bidirectionnel)
;   A480_S      : autoroute sud   (speed=7, class=4, toll=true, bidirectionnel)
;   D1075       : nationale       (speed=5, class=2, bidirectionnel)
;   Rue_Oneway  : rue sens unique (speed=3, class=1, DirIndicator=1)
;   Zone_Pietons: zone piétonne   (speed=1, class=0, denied_car+truck+bicycle)
;
; Route Level=1 (1 route isolée) :
;   Pont_Sud    : pont            (speed=4, class=2, Level=1)
;                 — Level=1 → noeuds isolés du réseau Level=0
;                 — (45.16,5.73) partagé en coords avec endpoint A480_S
;                   mais Level différent → pas de connexion
;
; Topologie Level=0 attendue :
;   6 RouteNodes, 11 RouteArcs
;   Noeud (45.18,5.73) = triple intersection A480_N / A480_S / D1075
;   Noeud (45.20,5.73) = endpoint A480_N nord
;   Noeud (45.16,5.73) = endpoint A480_S sud
;   Noeud (45.18,5.76) = intersection D1075 / Rue_Oneway
;   Noeud (45.19,5.76) = intersection Rue_Oneway / Zone_Pietons
;   Noeud (45.20,5.76) = endpoint Zone_Pietons
;
; Topologie Level=1 attendue :
;   2 RouteNodes Level=1, 2 RouteArcs Level=1 (Pont_Sud bidirectionnel)
;
; Validation tabAInfo NOD1 :
;   A480 (speed=7,class=4,toll=true,oneway=false) → tabAInfo = 0xC7
;   D1075 (speed=5,class=2,toll=false,oneway=false) → tabAInfo = 0x25
;   Rue_Oneway (speed=3,class=1,toll=false,oneway=true) → tabAInfo = 0x1B
;   Zone_Pietons (speed=1,class=0,toll=false,oneway=false) → tabAInfo = 0x01
;
; Validation access_mask NET1 :
;   Zone_Pietons : denied_car(0x0001) | denied_bicycle(0x0020) | denied_truck(0x0040) = 0x0061
;   → NET_FLAG_ACCESS (0x20) activé dans flags + 2 bytes access dans record NET1

[IMG ID]
Name=Routing Full Validation
ID=63240005
CodePage=1252
Levels=2
Level0=24
Level1=18
[END-IMG ID]

; Road 1: A480_N — autoroute nord (speed=7, class=4, toll=true, bidirectionnel)
; (45.18,5.73) → (45.20,5.73)
; Partage endpoint (45.18,5.73) avec A480_S et D1075 = noeud triple
[POLYLINE]
Type=0x01
Label=A480_N
Data0=(45.180000,5.730000),(45.200000,5.730000)
EndLevel=3
RoadID=1
RouteParam=7,4,0,1,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 2: A480_S — autoroute sud (speed=7, class=4, toll=true, bidirectionnel)
; (45.18,5.73) → (45.16,5.73)
; Partage endpoint (45.18,5.73) avec A480_N et D1075 = noeud triple
[POLYLINE]
Type=0x01
Label=A480_S
Data0=(45.180000,5.730000),(45.160000,5.730000)
EndLevel=3
RoadID=2
RouteParam=7,4,0,1,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 3: D1075 — nationale (speed=5, class=2, bidirectionnel)
; (45.18,5.73) → (45.18,5.76)
; Partage endpoint (45.18,5.73) avec A480_N et A480_S = noeud triple
[POLYLINE]
Type=0x02
Label=D1075
Data0=(45.180000,5.730000),(45.180000,5.760000)
EndLevel=3
RoadID=3
RouteParam=5,2,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=0
[END]

; Road 4: Rue_Oneway — sens unique (speed=3, class=1, DirIndicator=1)
; (45.18,5.76) → (45.19,5.76)
; Partage endpoint (45.18,5.76) avec D1075 = intersection
[POLYLINE]
Type=0x06
Label=Rue_Oneway
Data0=(45.180000,5.760000),(45.190000,5.760000)
EndLevel=2
RoadID=4
RouteParam=3,1,1,0,0,0,0,0,0,0,0,0
DirIndicator=1
Level=0
[END]

; Road 5: Zone_Pietons — zone piétonne (speed=1, class=0, denied_car+bicycle+truck)
; (45.19,5.76) → (45.20,5.76)
; Partage endpoint (45.19,5.76) avec Rue_Oneway = intersection
; RouteParam: speed=1, class=0, oneway=0, toll=0, emergency=0, delivery=0,
;             car=1, bus=0, taxi=0, pedestrian=0, bicycle=1, truck=1
; access_mask attendu = 0x0001 (car) | 0x0020 (bicycle) | 0x0040 (truck) = 0x0061
[POLYLINE]
Type=0x06
Label=Zone_Pietons
Data0=(45.190000,5.760000),(45.200000,5.760000)
EndLevel=2
RoadID=5
RouteParam=1,0,0,0,0,0,1,0,0,0,1,1
DirIndicator=0
Level=0
[END]

; Road 6: Pont_Sud — pont Level=1 (speed=4, class=2, bidirectionnel)
; (45.16,5.73) → (45.16,5.76)
; Level=1 → noeuds isolés du réseau Level=0
; (45.16,5.73) a les mêmes coords que l'endpoint A480_S mais Level différent
; → pas de connexion entre les réseaux Level=0 et Level=1
[POLYLINE]
Type=0x01
Label=Pont_Sud
Data0=(45.160000,5.730000),(45.160000,5.760000)
EndLevel=3
RoadID=6
RouteParam=4,2,0,0,0,0,0,0,0,0,0,0
DirIndicator=0
Level=1
[END]

; Non-routable polyline (rivière, pas d'attributs routage)
[POLYLINE]
Type=0x1A
Label=Drac
Data0=(45.180000,5.740000),(45.160000,5.740000)
EndLevel=3
[END]
