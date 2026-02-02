[IMG ID]
ID=12345678
Name=Polygon Invalid Coords Test
CodePage=1252
[END]
[POLYGON]
Type=0x0001
Label=Valid Polygon
Data0=(48.85,2.35),(48.85,2.36),(48.86,2.36),(48.86,2.35),(48.85,2.35)
[END]
[POLYGON]
Type=0x0002
Label=Only Two Points
Data0=(48.88,2.38),(48.89,2.39)
[END]
[POLYGON]
Type=0x0003
Label=Out Of Range Lat
Data0=(95.0,2.35),(95.1,2.36),(95.2,2.37)
[END]
[POLYGON]
Type=0x0004
Label=Valid After Invalid
Data0=(48.90,2.40),(48.90,2.41),(48.91,2.41),(48.91,2.40)
[END]
