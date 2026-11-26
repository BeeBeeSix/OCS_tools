import math
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.tri as mtri
from matplotlib.path import Path as MplPath

from contour_config import chaikin_smooth

def get_xy(v):
    if isinstance(v, dict):
        v = v.get("location", v)
        return v["x"], v["y"]
    return v[0], v[1]


def build_contour_entities(pts: np.ndarray, tri_arr, bnd_xy, cfg: dict, rot_key: str, log):
    """Triangulate TIN mesh, clip by max_edge & boundary, and build styled LwPolyline + Text entities."""
    triang = (
        mtri.Triangulation(pts[:, 0], pts[:, 1], triangles=tri_arr)
        if tri_arr is not None
        else mtri.Triangulation(pts[:, 0], pts[:, 1])
    )
    z_vals = pts[:, 2]
    tris = triang.triangles
    mask = np.zeros(len(tris), dtype=bool)

    max_edge = cfg["max_edge"]
    if max_edge > 0:
        p0, p1, p2 = pts[tris[:, 0], :2], pts[tris[:, 1], :2], pts[tris[:, 2], :2]
        e01 = np.hypot(p1[:, 0] - p0[:, 0], p1[:, 1] - p0[:, 1])
        e12 = np.hypot(p2[:, 0] - p1[:, 0], p2[:, 1] - p1[:, 1])
        e20 = np.hypot(p0[:, 0] - p2[:, 0], p0[:, 1] - p2[:, 1])
        mask |= (e01 > max_edge) | (e12 > max_edge) | (e20 > max_edge)

    if bnd_xy is not None and len(bnd_xy) >= 3:
        centroids = pts[tris, :2].mean(axis=1)
        inside = MplPath(bnd_xy).contains_points(centroids)
        mask |= ~inside
        log(f"Clipped TIN to boundary ({int(np.sum(~mask))} active triangles).")

    if np.any(mask):
        triang.set_mask(mask)

    minor_int, major_int = cfg["minor_int"], cfg["major_int"]
    z_min = math.floor(z_vals.min() / minor_int) * minor_int
    z_max = math.ceil(z_vals.max() / minor_int) * minor_int
    levels = np.arange(z_min, z_max + minor_int * 0.5, minor_int)

    fig, ax = plt.subplots()
    cs = ax.tricontour(triang, z_vals, levels=levels)

    entities = []
    major_count, minor_count, label_count = 0, 0, 0

    for level, segs in zip(cs.levels, cs.allsegs):
        is_major = np.isclose(round(level / major_int) * major_int, level, atol=1e-6)
        layer = cfg["maj_layer"] if is_major else cfg["min_layer"]
        color = cfg["maj_color"] if is_major else cfg["min_color"]
        lw = cfg["maj_lw"] if is_major else cfg["min_lw"]
        width = cfg["maj_width"] if is_major else cfg["min_width"]

        for seg in segs:
            if len(seg) < 2:
                continue
            smoothed = chaikin_smooth(seg, iterations=cfg["smooth_passes"])
            verts_2d = [[round(float(x), 4), round(float(y), 4)] for x, y in smoothed]

            ent = {"type": "LwPolyline", "layer": layer, "color": color, "lineweight": lw, "vertices": verts_2d,
                   "_z": round(float(level), 4)}  # stripped by the client; used to lift to true elevation
            if width > 0:
                ent["constant_width"] = width
            entities.append(ent)

            if is_major:
                major_count += 1
            else:
                minor_count += 1

            if is_major and cfg["add_labels"] and len(smoothed) >= 2:
                diffs = np.diff(smoothed, axis=0)
                total_len = float(np.sum(np.hypot(diffs[:, 0], diffs[:, 1])))
                if total_len >= cfg["lbl_min_len"]:
                    mid_idx = len(smoothed) // 2
                    ax_x, ax_y = get_xy(smoothed[max(0, mid_idx - 1)])
                    bx_x, bx_y = get_xy(smoothed[min(len(smoothed) - 1, mid_idx)])
                    mid_x, mid_y = 0.5 * (ax_x + bx_x), 0.5 * (ax_y + bx_y)
                    angle_deg = math.degrees(math.atan2(float(bx_y - ax_y), float(bx_x - ax_x)))
                    if angle_deg > 90.0:
                        angle_deg -= 180.0
                    elif angle_deg < -90.0:
                        angle_deg += 180.0

                    txt_ent = {
                        "type": "Text",
                        "layer": cfg["lbl_layer"],
                        "color": cfg["lbl_color"],
                        "position": [round(float(mid_x), 4), round(float(mid_y), 4)],
                        "height": cfg["lbl_height"],
                        "value": f"{level:.{cfg['lbl_decimals']}f}",
                        "_z": round(float(level), 4),
                    }
                    if rot_key:
                        txt_ent[rot_key] = round(angle_deg, 2)
                    entities.append(txt_ent)
                    label_count += 1

    plt.close(fig)
    log(f"Prepared {major_count} major, {minor_count} minor contours, and {label_count} labels.")
    return entities
