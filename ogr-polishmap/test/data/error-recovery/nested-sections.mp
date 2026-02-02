[IMG ID]
ID=12345678
Name=Nested Sections Test
CodePage=1252
[END]
[POI]
Type=0x0001
Label=Normal POI
Data0=(2.35,48.85)
[END]
[POI]
Type=0x0002
Label=POI with nested section marker
[POI]
Type=0x0003
Label=This should not happen
Data0=(2.36,48.86)
[END]
[POI]
Type=0x0004
Label=Valid After Nested
Data0=(2.37,48.87)
[END]
