; Test file: polygon-open-ring.mp
; Story 1.6 AC4 - POLYGON with open ring (first != last point)
; Should auto-close with CPLDebug log

[IMG ID]
ID=test-polygon-open-ring
Name=Open Ring Polygon Test
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POLYGON]
Type=0x4C
Label=Open Forest
Data0=(48.8566,2.3522),(48.8577,2.3533),(48.8588,2.3522)
; Note: Ring is open - no closing point back to first coordinate!
EndLevel=3
[END]
