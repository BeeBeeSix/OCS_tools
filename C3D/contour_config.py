import json
import numpy as np

# OCS_BIN = r"C:\Program Files\Open CAD Studio\OpenCADStudio.exe"
OCS_BIN = r"U:\GITHUB\MyOpenCADStudio\target\release\OpenCADStudio.exe"
ACI_PRESETS = {
    "1 - Red": (1, "#FF0000"),
    "2 - Yellow": (2, "#FFFF00"),
    "3 - Green": (3, "#00FF00"),
    "4 - Cyan": (4, "#00FFFF"),
    "5 - Blue": (5, "#0000FF"),
    "6 - Magenta": (6, "#FF00FF"),
    "7 - White/Black": (7, "#FFFFFF"),
    "8 - Dark Gray": (8, "#808080"),
    "9 - Light Gray": (9, "#C0C0C0"),
    "30 - Orange": (30, "#FF7F00"),
    "40 - Gold/Tan": (40, "#FFBF00"),
    "94 - Olive Green": (94, "#4C7F00"),
    "252 - Charcoal": (252, "#545454"),
}

LINEWEIGHT_OPTIONS = {
    "ByLayer (-1)": -1,
    "0.05 mm (5)": 5,
    "0.09 mm (9)": 9,
    "0.13 mm (13)": 13,
    "0.15 mm (15)": 15,
    "0.18 mm (18)": 18,
    "0.20 mm (20)": 20,
    "0.25 mm (25)": 25,
    "0.30 mm (30)": 30,
    "0.35 mm (35)": 35,
    "0.40 mm (40)": 40,
    "0.50 mm (50)": 50,
}


def parse_mcp_json(resp):
    first_item = next(iter(resp.content))
    text = first_item.text.strip()
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return {"ok": False, "raw_text": text}


def chaikin_smooth(pts, iterations=1):
    """Smooth an open or closed 2D polyline while keeping open endpoints pinned."""
    pts = np.asarray(pts, dtype=float)
    if iterations <= 0 or len(pts) < 3:
        return pts
    closed = np.allclose(pts[0], pts[-1], atol=1e-6)
    for _ in range(iterations):
        if len(pts) < 3:
            break
        q = 0.75 * pts[:-1] + 0.25 * pts[1:]
        r = 0.25 * pts[:-1] + 0.75 * pts[1:]
        new_pts = np.empty((len(q) + len(r), 2), dtype=float)
        new_pts[0::2] = q
        new_pts[1::2] = r
        if not closed:
            pts = np.vstack([pts[0], new_pts, pts[-1]])
        else:
            pts = np.vstack([new_pts, new_pts[0]])
    return pts
