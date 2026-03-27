; Polish Map fixture with mixed standard + extended Garmin types
[IMG ID]
Name=Extended Types Test
ID=63240099
CodePage=1252
Levels=2
Level0=24
Level1=18
TreeSize=3000
RgnLimit=1024
[END-IMG ID]

; Standard POI (type < 0xFFFF)
[POI]
Type=0x2C00
Label=Mairie
Data0=(45.1880,5.7245)
EndLevel=1
[END]

; Extended POI (antenna — 0x11503)
[POI]
Type=0x11503
Label=Antenne
Data0=(45.1900,5.7300)
EndLevel=1
[END]

; Standard polyline (route — 0x06)
[POLYLINE]
Type=0x06
Label=Route D1075
Data0=(45.2000,5.7000),(45.2100,5.7100),(45.2200,5.7200)
EndLevel=1
[END]

; Extended polyline (voie ferrée — 0x10c00)
[POLYLINE]
Type=0x10c00
Label=Voie Ferrée
Data0=(45.2300,5.7000),(45.2400,5.7100),(45.2500,5.7200)
EndLevel=1
[END]

; Standard polygon (bois — 0x50)
[POLYGON]
Type=0x50
Label=Forêt
Data0=(45.2000,5.7000),(45.2100,5.7000),(45.2100,5.7100),(45.2000,5.7100),(45.2000,5.7000)
EndLevel=1
[END]

; Extended polygon (bâtiment — 0x1101c)
[POLYGON]
Type=0x1101c
Label=Bâtiment
Data0=(45.1800,5.7200),(45.1810,5.7200),(45.1810,5.7210),(45.1800,5.7210),(45.1800,5.7200)
EndLevel=1
[END]
