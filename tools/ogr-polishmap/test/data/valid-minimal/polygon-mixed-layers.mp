; Test file: polygon-mixed-layers.mp
; Story 1.6 - Mixed layers: POI + POLYLINE + POLYGON

[IMG ID]
ID=test-mixed-layers
Name=Mixed Layers Test
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POI]
Type=0x2C00
Label=Town Hall
Data0=(48.8600,2.3400)
EndLevel=3
[END]

[POLYLINE]
Type=0x16
Label=Main Street
Data0=(48.8500,2.3300),(48.8550,2.3350),(48.8600,2.3400)
[END]

[POLYGON]
Type=0x4C
Label=City Park
Data0=(48.8620,2.3420),(48.8650,2.3450),(48.8680,2.3420),(48.8650,2.3390),(48.8620,2.3420)
[END]

[POI]
Type=0x4000
Label=Restaurant
Data0=(48.8630,2.3430)
[END]

[POLYLINE]
Type=0x20
Label=River
Data0=(48.8580,2.3500),(48.8620,2.3520),(48.8660,2.3540),(48.8700,2.3550)
[END]

[POLYGON]
Type=0x17
Label=Lake
Data0=(48.8550,2.3550),(48.8580,2.3580),(48.8610,2.3550),(48.8580,2.3520),(48.8550,2.3550)
[END]

[POI]
Type=0x6402
Label=Bridge
Data0=(48.8620,2.3520)
[END]
