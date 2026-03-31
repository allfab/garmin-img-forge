[IMG ID]
ID=12345678
Name=Polyline Invalid Coords Test
CodePage=1252
[END]
[POLYLINE]
Type=0x0001
Label=Valid Polyline
Data0=(48.85,2.35),(48.86,2.36),(48.87,2.37)
[END]
[POLYLINE]
Type=0x0002
Label=Only One Point
Data0=(48.88,2.38)
[END]
[POLYLINE]
Type=0x0003
Label=Out Of Range Lat
Data0=(95.0,2.35),(48.86,2.36)
[END]
[POLYLINE]
Type=0x0004
Label=Valid After Invalid
Data0=(48.89,2.39),(48.90,2.40)
[END]
