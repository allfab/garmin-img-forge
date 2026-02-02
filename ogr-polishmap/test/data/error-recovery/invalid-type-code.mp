[IMG ID]
ID=12345678
Name=Invalid Type Code Test
CodePage=1252
[END]
[POI]
Type=0x0001
Label=Valid Type
Data0=(2.35,48.85)
[END]
[POI]
Type=0xZZZZ
Label=Invalid Hex Type
Data0=(2.36,48.86)
[END]
[POI]
Type=notahex
Label=Non-Hex Type
Data0=(2.37,48.87)
[END]
[POI]
Type=0x0002
Label=Valid After Invalid
Data0=(2.38,48.88)
[END]
