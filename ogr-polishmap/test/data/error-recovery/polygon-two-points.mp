; Test file: polygon-two-points.mp
; Story 1.6 - POLYGON with only 2 points (invalid, should be skipped with warning)

[IMG ID]
ID=test-polygon-two-points
Name=Two Points Polygon Test
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POLYGON]
Type=0x4C
Label=Invalid Polygon
Data0=(48.8566,2.3522),(48.8577,2.3533)
; Only 2 points - not valid for POLYGON (minimum 3)
[END]

[POLYGON]
Type=0x17
Label=Valid Polygon After Invalid
Data0=(48.8600,2.3500),(48.8620,2.3520),(48.8640,2.3500),(48.8600,2.3500)
[END]
