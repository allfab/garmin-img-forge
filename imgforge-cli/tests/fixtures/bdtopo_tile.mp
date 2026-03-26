; Fixture pour validation E2E Story 13.6 — tuile type BDTOPO Isère
; 14 features : 6 polylines, 4 POI, 4 polygones — labels accentués, shield codes
[IMG ID]
Name=BDTOPO Isère D038
ID=63240038
CodePage=1252
Levels=2
Level0=24
Level1=18
[END-IMG ID]

; Routes (Type 0x06 = Minor Highway)
[POLYLINE]
Type=0x06
Label=D1075
Data0=(45.180,5.720),(45.185,5.725),(45.190,5.730),(45.195,5.735),(45.200,5.740)
EndLevel=3
[END]

[POLYLINE]
Type=0x06
Label=~[0x04]A480
Data0=(45.150,5.710),(45.160,5.715),(45.170,5.720)
EndLevel=4
[END]

[POLYLINE]
Type=0x06
Label=D523
Data0=(45.200,5.750),(45.205,5.755),(45.210,5.760)
EndLevel=3
[END]

[POLYLINE]
Type=0x02
Label=Chemin de la Châtaigneraie
Data0=(45.190,5.740),(45.192,5.742)
EndLevel=1
[END]

[POLYLINE]
Type=0x06
Label=Route Nationale
Data0=(45.220,5.770),(45.225,5.775)
EndLevel=2
[END]

; Route sans label (EndLevel=0 -> uniquement au niveau détaillé)
[POLYLINE]
Type=0x06
Data0=(45.182,5.722),(45.184,5.724)
EndLevel=0
[END]

; POI
[POI]
Type=0x2C00
Label=Mairie de Saint-Égrève
Data0=(45.1880,5.7245)
EndLevel=4
[END]

[POI]
Type=0x6400
Label=Col du Glandon
Data0=(45.2500,5.8100)
EndLevel=4
[END]

[POI]
Type=0x2800
Label=Carrefour D1075
Data0=(45.1920,5.7310)
EndLevel=3
[END]

[POI]
Type=0x6400
Label=Forêt domaniale
Data0=(45.2100,5.7900)
EndLevel=2
[END]

; Polygones
[POLYGON]
Type=0x50
Label=Forêt de Porte
Data0=(45.200,5.750),(45.210,5.750),(45.210,5.760),(45.200,5.760),(45.200,5.750)
EndLevel=4
[END]

[POLYGON]
Type=0x01
Label=Zone bâtie
Data0=(45.185,5.725),(45.190,5.725),(45.190,5.730),(45.185,5.730),(45.185,5.725)
EndLevel=3
[END]

[POLYGON]
Type=0x3C
Label=Lac de Montagnole
Data0=(45.215,5.765),(45.220,5.765),(45.220,5.770),(45.215,5.770),(45.215,5.765)
EndLevel=3
[END]

[POLYGON]
Type=0x13
Data0=(45.160,5.710),(45.165,5.710),(45.165,5.715),(45.160,5.715),(45.160,5.710)
EndLevel=2
[END]
