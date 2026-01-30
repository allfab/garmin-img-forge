; Test file: polygon-multiple.mp
; Story 1.6 - Multiple POLYGON features (5)

[IMG ID]
ID=test-polygon-multiple
Name=Multiple Polygon Test
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POLYGON]
Type=0x4C
Label=Forest 1
Data0=(48.8566,2.3522)
Data1=(48.8577,2.3533)
Data2=(48.8588,2.3522)
Data3=(48.8566,2.3522)
[END]

[POLYGON]
Type=0x17
Label=Water Body
Data0=(48.9000,2.3000)
Data1=(48.9100,2.3100)
Data2=(48.9200,2.3050)
Data3=(48.9000,2.3000)
EndLevel=5
[END]

[POLYGON]
Type=0x14
Label=Park Area
Data0=(48.8800,2.3200)
Data1=(48.8850,2.3250)
Data2=(48.8900,2.3200)
Data3=(48.8850,2.3150)
Data4=(48.8800,2.3200)
[END]

[POLYGON]
Type=0x50
Label=Building Zone
Data0=(48.8650,2.3450)
Data1=(48.8660,2.3460)
Data2=(48.8670,2.3450)
Data3=(48.8660,2.3440)
Data4=(48.8650,2.3450)
Levels=0-2
[END]

[POLYGON]
Type=0x4E
Label=Field
Data0=(48.8700,2.3500)
Data1=(48.8720,2.3530)
Data2=(48.8750,2.3500)
Data3=(48.8700,2.3500)
[END]
