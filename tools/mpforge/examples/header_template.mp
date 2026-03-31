[IMG ID]
; ==============================================================================
; Header Template Example for mpforge (Story 8.1)
; ==============================================================================
; This template can be used with the HEADER_TEMPLATE configuration option
; to standardize header settings across all exported tiles.
;
; Usage in bdtopo.yaml:
;   header:
;     template: "examples/header_template.mp"
;
; All tiles exported with this configuration will use this header.
; Template takes precedence over individual header fields.
; ==============================================================================

; Basic map information
Name=BDTOPO Production Map
ID=0
Copyright=IGN 2026 - mpforge

; Detail levels (4 zoom levels recommended for regional maps)
Levels=4
Level0=24
Level1=21
Level2=18
Level3=15

; Performance tuning (optimized for large datasets like BDTOPO)
TreeSize=3000
RgnLimit=1024

; Map appearance
Transparent=N
Marine=N

; Processing options (optimized for cGPSmapper compatibility)
Preprocess=F
LBLcoding=9
SimplifyLevel=2
LeftSideTraffic=N

; Custom fields (optional - can be used by specific GPS devices)
; Uncomment and customize as needed:
; DrawPriority=25
; MG=N
; TreMargin=0.00000
; TreSize=2048

; ==============================================================================
; END OF HEADER TEMPLATE
; ==============================================================================
; Note: Do not add [POI], [POLYLINE], or [POLYGON] sections to this template.
; This file is used only for the [IMG ID] header section.
; Feature data will be added by mpforge during tile export.
; ==============================================================================
