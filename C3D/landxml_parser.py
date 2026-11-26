import xml.etree.ElementTree as ET
from typing import Dict, List, Tuple


def parse_landxml_surfaces(xml_path: str) -> List[Dict]:
    """
    Parse <Surface> elements from a LandXML file.
    Supports both:
      1. Standard LandXML (<Definition><Pnts><P id="..."> + <Faces><F>id1 id2 id3</F>)
      2. Inline 3DFace LandXML (<Faces><F>n1 e1 z1 n2 e2 z2 n3 e3 z3</F> or child <P> tags)
    Returns a list of dicts with 'name', 'points' ({id: (easting, northing, elev)}),
    and 'faces' ([(id1, id2, id3), ...]).
    """
    tree = ET.parse(xml_path)
    root = tree.getroot()

    # Strip XML namespaces
    for elem in root.iter():
        if "}" in elem.tag:
            elem.tag = elem.tag.split("}", 1)[1]

    surfaces = []
    for surf in root.findall(".//Surface"):
        name = surf.attrib.get("name", "ImportedSurface")
        points: Dict[str, Tuple[float, float, float]] = {}
        coord_to_id: Dict[Tuple[float, float, float], str] = {}
        next_pid = 1

        def get_or_add_point(northing: float, easting: float, elev: float) -> str:
            nonlocal next_pid
            key = (round(easting, 6), round(northing, 6), round(elev, 6))
            if key not in coord_to_id:
                pid = str(next_pid)
                next_pid += 1
                coord_to_id[key] = pid
                points[pid] = (easting, northing, elev)
            return coord_to_id[key]

        # 1. Load explicit <Pnts><P id="..."> if present
        for p in surf.findall(".//Pnts/P"):
            pid = p.attrib.get("id")
            if not pid or not p.text:
                continue
            coords = [float(v) for v in p.text.strip().split()]
            if len(coords) >= 3:
                northing, easting, elev = coords[0], coords[1], coords[2]
                points[pid] = (easting, northing, elev)
                coord_to_id[(round(easting, 6), round(northing, 6), round(elev, 6))] = pid
                if pid.isdigit():
                    next_pid = max(next_pid, int(pid) + 1)

        # 2. Load <Faces><F> (handles both ID references and inline 3DFace coordinates)
        faces = []
        for f in surf.findall(".//Faces/F"):
            if f.attrib.get("i") == "1":
                continue

            # Case A: Inline child <P> elements inside <F>
            child_pts = f.findall("./P")
            if len(child_pts) >= 3:
                tri_ids = []
                for cp in child_pts[:3]:
                    vals = [float(v) for v in (cp.text or "").strip().split()]
                    if len(vals) >= 3:
                        tri_ids.append(get_or_add_point(vals[0], vals[1], vals[2]))
                if len(tri_ids) == 3:
                    faces.append(tuple(tri_ids))
                continue

            # Case B: Text content inside <F>
            tokens = (f.text or "").strip().split()
            if len(tokens) == 3 and all(t in points for t in tokens):
                # Standard point ID references
                faces.append((tokens[0], tokens[1], tokens[2]))
            elif len(tokens) >= 9:
                # Raw 3DFace inline coordinates: N1 E1 Z1 N2 E2 Z2 N3 E3 Z3
                vals = [float(v) for v in tokens[:9]]
                id1 = get_or_add_point(vals[0], vals[1], vals[2])
                id2 = get_or_add_point(vals[3], vals[4], vals[5])
                id3 = get_or_add_point(vals[6], vals[7], vals[8])
                faces.append((id1, id2, id3))

        surfaces.append({
            "name": name,
            "points": points,
            "faces": faces,
        })

    return surfaces


def export_to_standard_landxml(surface: Dict, out_xml_path: str, swap_xy: bool = True) -> None:
    """
    Write a clean, standard LandXML 1.2 <Surface> file with deduplicated <Pnts> and <Faces>.
    """
    landxml = ET.Element(
        "LandXML",
        {
            "xmlns": "http://landxml.org",
            "version": "1.2",
            "date": "2026-11-26",
            "time": "12:00:00",
        },
    )
    units = ET.SubElement(landxml, "Units")
    ET.SubElement(
        units,
        "Metric",
        {"areaUnit": "squareMeter", "linearUnit": "meter", "volumeUnit": "cubicMeter", "angularUnit": "decimal degrees"},
    )

    surfaces_el = ET.SubElement(landxml, "Surfaces")
    surf_el = ET.SubElement(surfaces_el, "Surface", {"name": surface["name"]})
    defn_el = ET.SubElement(surf_el, "Definition", {"surfType": "TIN"})

    pnts_el = ET.SubElement(defn_el, "Pnts")
    for pid, (x, y, z) in surface["points"].items():
        p_el = ET.SubElement(pnts_el, "P", {"id": str(pid)})
        # LandXML expects Northing Easting Elevation
        northing, easting = (y, x) if swap_xy else (x, y)
        p_el.text = f"{northing:.6f} {easting:.6f} {z:.6f}"

    faces_el = ET.SubElement(defn_el, "Faces")
    for id1, id2, id3 in surface["faces"]:
        f_el = ET.SubElement(faces_el, "F")
        f_el.text = f"{id1} {id2} {id3}"

    tree = ET.ElementTree(landxml)
    ET.indent(tree, space="  ")
    tree.write(out_xml_path, encoding="utf-8", xml_declaration=True)
