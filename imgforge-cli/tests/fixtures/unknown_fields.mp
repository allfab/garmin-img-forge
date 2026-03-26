; Polish Map file with unknown/undocumented fields for forward-compatibility testing
[IMG ID]
Name=Unknown Fields Test
ID=63240004
CodePage=1252
Levels=2
Level0=24
Level1=18
; Known extended fields (Preprocess, LBLcoding, etc.) — no dedicated struct field at this stage,
; stored in other_fields for forward-compatibility (see task 4.3)
Preprocess=F
LBLcoding=6
Transparent=N
SimplifyLevel=0
Marine=N
LeftSideTraffic=N
CustomVendorField=custom_value_123
AnotherUnknownField=hello_world
[END-IMG ID]

[POI]
Type=0x2C00
Label=Test POI
Data0=(45.1880,5.7245)
EndLevel=4
CustomPOIAttribute=poi_custom_value
ExtraField=extra
[END]

[POLYLINE]
Type=0x01
Data0=(45.2000,5.7000),(45.2100,5.7100)
CustomLineField=line_value
[END]

[POLYGON]
Type=0x50
Data0=(45.2000,5.7000),(45.2100,5.7000),(45.2100,5.7100),(45.2000,5.7100),(45.2000,5.7000)
CustomPolygonField=polygon_value
[END]
