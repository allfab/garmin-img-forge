[IMG ID]
ID=12345678
Name=Invalid Section Test
CodePage=1252
[END]
[POI]
Type=0x0001
Label=Valid POI
Data0=(2.35,48.85)
[END]
[POI]
; Missing Type - malformed section
Label=Invalid POI
Data0=(2.36,48.86)
[END]
[POI]
Type=0x0002
Label=Another Valid POI
Data0=(2.37,48.87)
[END]
