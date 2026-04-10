#!/usr/bin/env python3
"""
Parse un fichier TYP texte (I2023100.txt) et génère une page Markdown
de référence avec des rendus SVG inline pour chaque style.
"""

import argparse
import html
import re
import sys
from collections import Counter
from datetime import datetime
from pathlib import Path


def parse_typ_file(filepath):
    """Parse le fichier TYP et retourne les sections polygon, line, point."""
    with open(filepath, "r", encoding="cp1252") as f:
        content = f.read()

    sections = {"polygon": [], "line": [], "point": []}

    # Split on section headers
    pattern = r'\[(_polygon|_line|_point)\]\s*\n(.*?)\[end\]'
    for match in re.finditer(pattern, content, re.DOTALL | re.IGNORECASE):
        section_type = match.group(1).lower().lstrip("_")
        body = match.group(2)
        parsed = parse_section(body, section_type)
        if parsed:
            sections[section_type].append(parsed)

    return sections


def parse_section(body, section_type):
    """Parse une section individuelle et extrait les propriétés."""
    info = {
        "type": None,
        "subtype": None,
        "grmn_type": None,
        "strings": {},
        "xpm": None,
        "day_xpm": None,
        "night_xpm": None,
        "line_width": None,
        "border_width": None,
        "use_orientation": None,
        "custom_color": None,
        "day_custom_color": None,
        "night_custom_color": None,
        "font_style": None,
        "extended_labels": None,
        "contour_color": None,
    }

    lines = body.split("\n")
    i = 0
    while i < len(lines):
        line = lines[i].strip()

        if line.startswith("Type="):
            info["type"] = line.split("=", 1)[1].strip()
        elif line.startswith("SubType="):
            info["subtype"] = line.split("=", 1)[1].strip()
        elif line.startswith(";GRMN_TYPE:"):
            info["grmn_type"] = line[len(";GRMN_TYPE:"):].strip()
        elif line.startswith("String"):
            m = re.match(r'String(\d+)=(.+)', line)
            if m:
                info["strings"][int(m.group(1))] = m.group(2).strip()
        elif line.startswith("LineWidth="):
            info["line_width"] = int(line.split("=", 1)[1].strip())
        elif line.startswith("BorderWidth="):
            info["border_width"] = int(line.split("=", 1)[1].strip())
        elif line.startswith("UseOrientation="):
            info["use_orientation"] = line.split("=", 1)[1].strip()
        elif line.startswith("CustomColor="):
            info["custom_color"] = line.split("=", 1)[1].strip()
        elif line.startswith("DaycustomColor:"):
            info["day_custom_color"] = line.split(":", 1)[1].strip()
        elif line.startswith("NightcustomColor:"):
            info["night_custom_color"] = line.split(":", 1)[1].strip()
        elif line.startswith("FontStyle="):
            info["font_style"] = line.split("=", 1)[1].strip()
        elif line.startswith("ExtendedLabels="):
            info["extended_labels"] = line.split("=", 1)[1].strip()
        elif line.startswith("ContourColor="):
            info["contour_color"] = line.split("=", 1)[1].strip()
        elif line.startswith("Xpm=") or line.startswith("DayXpm=") or line.startswith("NightXpm="):
            xpm_key = "xpm"
            if line.startswith("DayXpm="):
                xpm_key = "day_xpm"
            elif line.startswith("NightXpm="):
                xpm_key = "night_xpm"

            # Parse XPM header
            m = re.match(r'(?:Day|Night)?Xpm="(\d+)\s+(\d+)\s+(\d+)\s+(\d+)"', line)
            if m:
                w, h, ncolors, cpp = int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4))
                colors = {}
                pixel_rows = []

                # Read color definitions
                j = i + 1
                for _ in range(ncolors):
                    while j < len(lines):
                        cl = lines[j].strip()
                        j += 1
                        if cl.startswith('"'):
                            # Parse color: "KEY c #RRGGBB" or "KEY c none"
                            # KEY is cpp characters (1 or 2), separated by tab or spaces
                            cm = re.match(r'"(.{' + str(cpp) + r'})\s+c\s+(#[0-9A-Fa-f]{6}|none)"', cl)
                            if not cm:
                                cm = re.match(r'"(.{' + str(cpp) + r'})\tc\s+(#[0-9A-Fa-f]{6}|none)"', cl)
                            if cm:
                                colors[cm.group(1)] = cm.group(2)
                            break
                        elif cl == "" or cl.startswith(";"):
                            continue
                        else:
                            break

                # Read pixel rows
                if w > 0 and h > 0:
                    for _ in range(h):
                        while j < len(lines):
                            cl = lines[j].strip()
                            j += 1
                            if cl.startswith('"') and not cl.startswith('"') or cl.startswith('"'):
                                # Extract pixel data between quotes
                                pm = re.match(r'"([^"]*)"', cl)
                                if pm:
                                    pixel_rows.append(pm.group(1))
                                break
                            elif cl.startswith(";"):
                                continue
                            else:
                                break

                info[xpm_key] = {
                    "width": w,
                    "height": h,
                    "ncolors": ncolors,
                    "cpp": cpp,
                    "colors": colors,
                    "pixels": pixel_rows,
                }
                i = j - 1

        i += 1

    return info


def xpm_to_svg(xpm_data, scale=2, is_line=False, line_width=None):
    """Convertit des données XPM en SVG inline."""
    if not xpm_data:
        return None

    w = xpm_data["width"]
    h = xpm_data["height"]
    colors = xpm_data["colors"]
    pixels = xpm_data["pixels"]

    if w == 0 and h == 0:
        # Solid color fill - render a simple rectangle
        color1 = list(colors.values())[0] if colors else "#CCCCCC"
        if color1 == "none":
            color1 = list(colors.values())[1] if len(colors) > 1 else "#CCCCCC"
        svg_w = 48
        svg_h = 16 if is_line else 48
        svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{svg_w}" height="{svg_h}" viewBox="0 0 {svg_w} {svg_h}">'
        if is_line:
            lw = line_width or 2
            y = svg_h // 2
            svg += f'<line x1="0" y1="{y}" x2="{svg_w}" y2="{y}" stroke="{color1}" stroke-width="{lw}"/>'
        else:
            svg += f'<rect width="{svg_w}" height="{svg_h}" fill="{color1}"/>'
        svg += '</svg>'
        return svg

    if not pixels:
        return None

    # For lines, tile 2x horizontally for better visual
    tile_x = 2 if is_line else 1
    tile_y = 1
    total_w = w * tile_x
    total_h = h * tile_y

    svg_w = total_w * scale
    svg_h = total_h * scale

    # Cap size
    max_dim = 96
    if svg_w > max_dim:
        scale = max(1, max_dim // total_w)
        svg_w = total_w * scale
        svg_h = total_h * scale

    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{svg_w}" height="{svg_h}" viewBox="0 0 {total_w} {total_h}" shape-rendering="crispEdges">'

    # Draw pixels (handle cpp > 1)
    cpp = xpm_data.get("cpp", 1)

    # Background - determine by pixel frequency (most common color = background)
    char_counts = Counter()
    for row in pixels:
        for ci in range(0, len(row), cpp):
            key = row[ci:ci + cpp]
            char_counts[key] += 1

    bg_color = None
    if char_counts:
        most_common_key = char_counts.most_common(1)[0][0]
        bg_color = colors.get(most_common_key)

    if bg_color and bg_color != "none":
        svg += f'<rect width="{total_w}" height="{total_h}" fill="{bg_color}"/>'
    else:
        # Transparent background → render as white (map background on Garmin)
        # Reset bg_color to None so the pixel loop draws all non-none colors
        bg_color = None
        svg += f'<rect width="{total_w}" height="{total_h}" fill="#FFFFFF"/>'

    for ty in range(tile_y):
        for row_idx, row in enumerate(pixels):
            y = ty * h + row_idx
            if not row:
                continue
            x = 0
            for tx in range(tile_x):
                for ci in range(0, len(row), cpp):
                    key = row[ci:ci + cpp]
                    color = colors.get(key)
                    if color and color != "none" and color != bg_color:
                        svg += f'<rect x="{x}" y="{y}" width="1" height="1" fill="{color}"/>'
                    x += 1

    svg += '</svg>'
    return svg


def point_xpm_to_svg(xpm_data, scale=2):
    """Convertit un XPM de point en SVG."""
    if not xpm_data:
        return None

    w = xpm_data["width"]
    h = xpm_data["height"]
    colors = xpm_data["colors"]
    pixels = xpm_data["pixels"]

    if w == 0 or h == 0 or not pixels:
        return None

    svg_w = w * scale
    svg_h = h * scale

    svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{svg_w}" height="{svg_h}" viewBox="0 0 {w} {h}" shape-rendering="crispEdges">'

    # Transparent background
    svg += f'<rect width="{w}" height="{h}" fill="#FFFFFF"/>'

    cpp = xpm_data.get("cpp", 1)
    for row_idx, row in enumerate(pixels):
        if not row:
            continue
        col = 0
        for ci in range(0, len(row), cpp):
            key = row[ci:ci + cpp]
            color = colors.get(key)
            if color and color != "none":
                svg += f'<rect x="{col}" y="{row_idx}" width="1" height="1" fill="{color}"/>'
            col += 1

    svg += '</svg>'
    return svg


def get_label(info):
    """Récupère le label le plus pertinent (FR > EN > DE)."""
    # Priorité : String2 (FR 0x01), String1 (EN 0x04), String4 (0x00), String3
    for key in [2, 1, 4, 3]:
        if key in info["strings"]:
            val = info["strings"][key]
            # Remove language prefix like "0x04,"
            if "," in val:
                val = val.split(",", 1)[1]
            return val
    return ""


def format_type_code(info):
    """Formate le code type complet."""
    t = info["type"]
    if info["subtype"]:
        return f"{t} / {info['subtype']}"
    return t


def format_grmn_path(grmn):
    """Extrait une description lisible du chemin GRMN_TYPE."""
    if not grmn:
        return ""
    parts = grmn.split("/")
    if len(parts) >= 3:
        category = parts[0].strip()
        name = parts[2].strip()
        return f"{category} — {name}"
    return grmn


def get_colors_display(info):
    """Retourne les couleurs utilisées."""
    colors = set()

    for xpm_key in ["xpm", "day_xpm", "night_xpm"]:
        xpm = info.get(xpm_key)
        if xpm and "colors" in xpm:
            for c in xpm["colors"].values():
                if c != "none":
                    colors.add(c)

    if info.get("day_custom_color"):
        colors.add(info["day_custom_color"])

    return list(colors)


def color_swatch_html(color):
    """Génère un petit carré de couleur HTML."""
    return f'<span style="display:inline-block;width:14px;height:14px;background:{color};border:1px solid #ccc;vertical-align:middle;margin-right:2px;border-radius:2px;"></span>'


def render_table_section(items, section_type):
    """Génère un tableau HTML pour une section de styles."""
    rows = []
    for info in items:
        if section_type == "point":
            xpm = info.get("day_xpm") or info.get("xpm")
            svg = point_xpm_to_svg(xpm, scale=2)
        elif section_type == "line":
            lw = info.get("line_width")
            svg = xpm_to_svg(info["xpm"], scale=2, is_line=True, line_width=lw)
        else:
            svg = xpm_to_svg(info["xpm"], scale=2, is_line=False)

        type_code = format_type_code(info)
        grmn = format_grmn_path(info["grmn_type"])
        label = get_label(info)

        if label and grmn:
            desc = f"<strong>{html.escape(label)}</strong><br><small>{html.escape(grmn)}</small>"
        elif label:
            desc = html.escape(label)
        elif grmn:
            desc = html.escape(grmn)
        else:
            desc = "—"

        colors = get_colors_display(info)
        color_cells = " ".join(f'{color_swatch_html(c)} <code>{c}</code>' for c in colors)

        svg_cell = svg if svg else "—"

        rows.append(
            f'  <tr>\n'
            f'    <td style="text-align:center;vertical-align:middle;">{svg_cell}</td>\n'
            f'    <td><code>{html.escape(type_code)}</code></td>\n'
            f'    <td>{desc}</td>\n'
            f'    <td>{color_cells}</td>\n'
            f'  </tr>'
        )

    table = (
        '<table>\n'
        '  <thead>\n'
        '    <tr>\n'
        '      <th style="text-align:center;">Rendu</th>\n'
        '      <th>Type</th>\n'
        '      <th>Description</th>\n'
        '      <th>Couleurs</th>\n'
        '    </tr>\n'
        '  </thead>\n'
        '  <tbody>\n'
        + "\n".join(rows) + "\n"
        '  </tbody>\n'
        '</table>'
    )
    return table


def generate_markdown(sections, output_path, source_name="I2023100.txt"):
    """Génère la page Markdown complète avec tableaux HTML."""

    stem = Path(source_name).stem
    md = []
    md.append(f"# Styles TYP — {stem}")
    md.append("")
    md.append(f"Catalogue exhaustif des styles définis dans le fichier TYP `{source_name}`.")
    md.append("Chaque style est accompagné d'un aperçu du rendu (motif XPM) et de ses couleurs.")
    md.append("")
    md.append(f"> **Fichier source** : `{source_name}`")
    md.append(f"> **Généré le** : {datetime.now().strftime('%d/%m/%Y à %H:%M')}")
    md.append("")
    md.append("---")
    md.append("")

    # Table of contents
    md.append("## Sommaire")
    md.append("")
    md.append(f"- **Polygones** : {len(sections['polygon'])} styles")
    md.append(f"- **Lignes** : {len(sections['line'])} styles")
    md.append(f"- **Points** : {len(sections['point'])} styles")
    md.append(f"- **Total** : {sum(len(v) for v in sections.values())} styles")
    md.append("")
    md.append("---")
    md.append("")

    # === POLYGONS ===
    md.append("## Polygones")
    md.append("")
    polygons_sorted = sorted(sections["polygon"], key=lambda x: x["type"])
    md.append(render_table_section(polygons_sorted, "polygon"))
    md.append("")
    md.append("---")
    md.append("")

    # === LINES ===
    md.append("## Lignes")
    md.append("")
    lines_sorted = sorted(sections["line"], key=lambda x: x["type"])
    md.append(render_table_section(lines_sorted, "line"))
    md.append("")
    md.append("---")
    md.append("")

    # === POINTS ===
    md.append("## Points")
    md.append("")
    points_sorted = sorted(sections["point"], key=lambda x: (x["type"], x.get("subtype") or ""))
    md.append(render_table_section(points_sorted, "point"))
    md.append("")

    with open(output_path, "w", encoding="utf-8") as f:
        f.write("\n".join(md))

    print(f"Page générée : {output_path}")
    print(f"  Polygones : {len(sections['polygon'])}")
    print(f"  Lignes    : {len(sections['line'])}")
    print(f"  Points    : {len(sections['point'])}")
    print(f"  Total     : {sum(len(v) for v in sections.values())}")


def main():
    base = Path(__file__).resolve().parent.parent

    parser = argparse.ArgumentParser(
        description="Génère une page Markdown de référence visuelle à partir d'un fichier TYP texte."
    )
    parser.add_argument(
        "input",
        nargs="?",
        default=str(base / "pipeline" / "resources" / "typfiles" / "I2023100.txt"),
        help="Fichier TYP texte en entrée (défaut: pipeline/resources/typfiles/I2023100.txt)",
    )
    parser.add_argument(
        "-o", "--output",
        default=None,
        help="Fichier Markdown en sortie (défaut: site/docs/reference/styles-typ.md)",
    )
    args = parser.parse_args()

    typ_file = Path(args.input)
    if args.output:
        output = Path(args.output)
    else:
        output = base / "site" / "docs" / "reference" / "styles-typ.md"

    if not typ_file.exists():
        print(f"Fichier TYP introuvable : {typ_file}")
        sys.exit(1)

    output.parent.mkdir(parents=True, exist_ok=True)

    print(f"Parsing {typ_file}...")
    sections = parse_typ_file(str(typ_file))
    generate_markdown(sections, str(output), source_name=typ_file.name)


if __name__ == "__main__":
    main()
