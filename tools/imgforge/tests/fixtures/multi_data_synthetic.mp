; Fixture multi-level Data — Tech-spec #1 imgforge multi-level Data
; Démontre la capacité réelle du pipeline à honorer DataN par niveau.
;
; 3 niveaux Garmin (24, 22, 20).
; Chaque feature a une géométrie distincte par bucket :
;   - Data0 = détaillé
;   - Data1 = simplifié (polyline et polygon)
;   - Data2 = très simplifié (polyline)

[IMG ID]
ID=99990001
Name=multi-data-synthetic
CodePage=1252
Levels=24,22,20
Level0=24
Level1=22
Level2=20
[END-IMG ID]

[POLYLINE]
Type=0x06
Label=Route detaillee
EndLevel=2
Data0=(48.5734,7.7521),(48.5740,7.7530),(48.5750,7.7540),(48.5760,7.7550),(48.5770,7.7560)
Data1=(48.5734,7.7521),(48.5750,7.7540),(48.5770,7.7560)
Data2=(48.5734,7.7521),(48.5770,7.7560)
[END]

[POLYLINE]
Type=0x01
Label=Route Data0 only
EndLevel=2
Data0=(48.6000,7.8000),(48.6010,7.8010),(48.6020,7.8020)
[END]

[POLYGON]
Type=0x03
Label=Foret multi-level
EndLevel=2
Data0=(48.5500,7.7000),(48.5500,7.7100),(48.5510,7.7100),(48.5510,7.7050),(48.5505,7.7000)
Data1=(48.5500,7.7000),(48.5500,7.7100),(48.5510,7.7100),(48.5500,7.7000)
[END]
