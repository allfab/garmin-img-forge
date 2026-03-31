[IMG ID]
ID=12345678
Name=Corrupt Geometry Test
CodePage=1252
[END]
[POI]
Type=0x0001
Label=Valid POI
Data0=(48.85,2.35)
[END]
[POI]
Type=0x0001
Label=Invalid Lat Over 90
Data0=(95.0,2.35)
[END]
[POI]
Type=0x0001
Label=Another Valid
Data0=(48.86,2.36)
[END]
[POI]
Type=0x0001
Label=Invalid Lon Over 180
Data0=(48.87,200.0)
[END]
[POI]
Type=0x0001
Label=Valid After Invalid
Data0=(48.88,2.37)
[END]
