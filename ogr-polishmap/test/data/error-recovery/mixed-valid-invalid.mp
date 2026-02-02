[IMG ID]
ID=12345678
Name=Mixed Valid Invalid Test
CodePage=1252
[END]
; Valid POI
[POI]
Type=0x0001
Label=Valid POI 1
Data0=(48.85,2.35)
[END]
; Invalid geometry - lat > 90
[POI]
Type=0x0002
Label=Invalid Lat
Data0=(95.0,2.36)
[END]
; Valid POI
[POI]
Type=0x0003
Label=Valid POI 2
Data0=(48.87,2.37)
[END]
; Valid Polyline
[POLYLINE]
Type=0x0001
Label=Valid Road
Data0=(48.90,2.40),(48.91,2.41),(48.92,2.42)
[END]
; Invalid Polyline (1 point)
[POLYLINE]
Type=0x0002
Label=Single Point Road
Data0=(48.93,2.43)
[END]
; Valid Polyline
[POLYLINE]
Type=0x0003
Label=Another Road
Data0=(48.94,2.44),(48.95,2.45)
[END]
; Valid Polygon
[POLYGON]
Type=0x0001
Label=Valid Area
Data0=(48.50,2.50),(48.50,2.51),(48.51,2.51),(48.51,2.50)
[END]
; Invalid Polygon (2 points)
[POLYGON]
Type=0x0002
Label=Line Not Polygon
Data0=(48.52,2.52),(48.53,2.53)
[END]
; Valid Polygon
[POLYGON]
Type=0x0003
Label=Another Area
Data0=(48.54,2.54),(48.54,2.55),(48.55,2.55),(48.55,2.54)
[END]
