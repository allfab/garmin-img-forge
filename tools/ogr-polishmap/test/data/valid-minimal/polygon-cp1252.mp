; Test file: polygon-cp1252.mp
; Story 1.6 - POLYGON with CP1252 encoded labels
; Contains special characters in CP1252 encoding

[IMG ID]
ID=test-polygon-cp1252
Name=CP1252 Encoding Test
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POLYGON]
Type=0x4C
Label=Forêt Réservée
Data0=(48.8566,2.3522),(48.8577,2.3533),(48.8588,2.3522),(48.8566,2.3522)
EndLevel=3
[END]

[POLYGON]
Type=0x17
Label=Zürich Grünfläche
Data0=(48.9000,2.3000),(48.9100,2.3100),(48.9200,2.3000),(48.9000,2.3000)
[END]
