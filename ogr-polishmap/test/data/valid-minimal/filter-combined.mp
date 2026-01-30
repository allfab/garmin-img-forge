; Polish Map Format - Combined Filter Test
; Mix of POI, POLYLINE, and POLYGON with various attributes
; For testing combined spatial + attribute filters

[IMG ID]
Name=Filter Combined Test
CodePage=1252
[END]

; === POIs in different regions ===
; North region (lat > 48.5)
[POI]
Type=0x2C00
Label=Restaurant Nord 1
Data0=(48.7,2.2)
EndLevel=3
[END]

[POI]
Type=0x2B00
Label=Hotel Nord 2
Data0=(48.8,2.5)
EndLevel=3
[END]

[POI]
Type=0x4000
Label=Attraction Nord 3
Data0=(48.6,2.8)
EndLevel=4
[END]

; South region (lat < 48.5)
[POI]
Type=0x2C00
Label=Restaurant Sud 1
Data0=(48.2,2.3)
EndLevel=3
[END]

[POI]
Type=0x2B00
Label=Hotel Sud 2
Data0=(48.1,2.6)
EndLevel=3
[END]

[POI]
Type=0x4000
Label=Attraction Sud 3
Data0=(48.3,2.1)
EndLevel=4
[END]

; === POLYLINEs in different regions ===
; North region road
[POLYLINE]
Type=0x16
Label=Route Nord Primary
Data0=(48.7,2.0)
Data1=(48.8,2.2)
Data2=(48.9,2.4)
EndLevel=4
[END]

[POLYLINE]
Type=0x0D
Label=Trail Nord Secondary
Data0=(48.6,2.5)
Data1=(48.7,2.7)
Data2=(48.8,2.9)
EndLevel=2
[END]

; South region road
[POLYLINE]
Type=0x16
Label=Route Sud Primary
Data0=(48.1,2.0)
Data1=(48.2,2.2)
Data2=(48.3,2.4)
EndLevel=4
[END]

[POLYLINE]
Type=0x1E
Label=Street Sud Tertiary
Data0=(48.0,2.5)
Data1=(48.2,2.7)
Data2=(48.4,2.8)
EndLevel=2
[END]

; === POLYGONs in different regions ===
; North region area
[POLYGON]
Type=0x17
Label=Park Nord
Data0=(48.7,2.1)
Data1=(48.8,2.1)
Data2=(48.8,2.3)
Data3=(48.7,2.3)
Data4=(48.7,2.1)
EndLevel=3
[END]

[POLYGON]
Type=0x50
Label=Forest Nord
Data0=(48.6,2.6)
Data1=(48.7,2.6)
Data2=(48.7,2.8)
Data3=(48.6,2.8)
Data4=(48.6,2.6)
EndLevel=3
[END]

; South region area
[POLYGON]
Type=0x17
Label=Park Sud
Data0=(48.2,2.2)
Data1=(48.3,2.2)
Data2=(48.3,2.4)
Data3=(48.2,2.4)
Data4=(48.2,2.2)
EndLevel=3
[END]

[POLYGON]
Type=0x3C
Label=Lake Sud
Data0=(48.1,2.5)
Data1=(48.2,2.5)
Data2=(48.2,2.7)
Data3=(48.1,2.7)
Data4=(48.1,2.5)
EndLevel=3
[END]
