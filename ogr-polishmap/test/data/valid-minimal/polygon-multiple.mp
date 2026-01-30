; Test file: polygon-multiple.mp
; Story 1.6 - Multiple POLYGON features (5)

[IMG ID]
ID=test-polygon-multiple
Name=Multiple Polygon Test
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POLYGON]
Type=0x4C
Label=Forest 1
Data0=(48.8566,2.3522),(48.8577,2.3533),(48.8588,2.3522),(48.8566,2.3522)
[END]

[POLYGON]
Type=0x17
Label=Water Body
Data0=(48.9000,2.3000),(48.9100,2.3100),(48.9200,2.3050),(48.9000,2.3000)
EndLevel=5
[END]

[POLYGON]
Type=0x14
Label=Park Area
Data0=(48.8800,2.3200),(48.8850,2.3250),(48.8900,2.3200),(48.8850,2.3150),(48.8800,2.3200)
[END]

[POLYGON]
Type=0x50
Label=Building Zone
Data0=(48.8650,2.3450),(48.8660,2.3460),(48.8670,2.3450),(48.8660,2.3440),(48.8650,2.3450)
Levels=0-2
[END]

[POLYGON]
Type=0x4E
Label=Field
Data0=(48.8700,2.3500),(48.8720,2.3530),(48.8750,2.3500),(48.8700,2.3500)
[END]
