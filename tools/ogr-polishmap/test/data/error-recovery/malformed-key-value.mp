[IMG ID]
ID=12345678
Name=Malformed Key Value Test
CodePage=1252
[END]
[POI]
Type=0x0001
Label=Valid POI
Data0=(2.35,48.85)
[END]
[POI]
Type=0x0002
This line has no equals sign
Label=Should Still Parse
Data0=(2.36,48.86)
[END]
[POI]
Type=0x0003
=Value without key
Label=Another Valid
Data0=(2.37,48.87)
[END]
