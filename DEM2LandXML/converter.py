from datetime import datetime, timezone
from matplotlib.path import Path as MplPath
import numpy as np
import rasterio


def fmt(val):
    s = f"{val:.4f}".rstrip("0").rstrip(".")
    return "0" if s == "-0" else s


def convert_dem_to_landxml(
    in_file,
    out_file,
    surf_name="DEM_Surface",
    step=1,
    poly_coords=None,
    status_callback=None,
):
    def update(msg):
        if status_callback:
            status_callback(msg)

    with rasterio.open(in_file) as src:
        elevation = src.read(1)[::step, ::step]
        nodata = src.nodata
        transform = src.transform * src.transform.scale(step, step)
        rows, cols = elevation.shape

        valid_mask = ~np.isnan(elevation)
        if nodata is not None:
            valid_mask &= elevation != nodata

        col_idx, row_idx = np.meshgrid(np.arange(cols), np.arange(rows))
        xs, ys = rasterio.transform.xy(
            transform, row_idx, col_idx, offset="center"
        )
        xs = np.array(xs).reshape(rows, cols)
        ys = np.array(ys).reshape(rows, cols)

        # Apply polygon crop if 3 or more vertices were drawn
        if poly_coords and len(poly_coords) >= 3:
            update("Applying polygon crop mask...")
            pts = np.column_stack((xs.ravel(), ys.ravel()))
            inside = MplPath(poly_coords).contains_points(pts)
            valid_mask &= inside.reshape(rows, cols)

        point_ids = np.zeros((rows, cols), dtype=np.int64)
        valid_count = int(np.count_nonzero(valid_mask))
        if valid_count == 0:
            raise ValueError(
                "No valid elevation points found inside the selection."
            )

        point_ids[valid_mask] = np.arange(1, valid_count + 1)
        v_ids, v_x, v_y, v_z = (
            point_ids[valid_mask],
            xs[valid_mask],
            ys[valid_mask],
            elevation[valid_mask],
        )

        now = datetime.now(timezone.utc)
        with open(out_file, "w", encoding="utf-8") as f:
            f.write('<?xml version="1.0" encoding="UTF-8"?>\n')
            f.write(
                '<LandXML xmlns="http://landxml.org" '
                f'version="1.2" date="{now:%Y-%m-%d}" time="{now:%H:%M:%S}" readOnly="false">\n'
            )
            f.write(
                '  <Units>\n    <Metric areaUnit="squareMeter" linearUnit="meter" '
                'volumeUnit="cubicMeter" temperatureUnit="celsius" '
                'pressureUnit="milliBars" diameterUnit="millimeter" '
                'angularUnit="decimal degrees" directionUnit="decimal degrees"/>\n  </Units>\n'
            )

            update(f"Writing {valid_count:,} CgPoints...")
            f.write("  <CgPoints>\n")
            for pid, y, x, z in zip(v_ids, v_y, v_x, v_z):
                f.write(
                    f'    <CgPoint name="{pid}" code="DEM">{fmt(y)} {fmt(x)} {fmt(z)}</CgPoint>\n'
                )
            f.write("  </CgPoints>\n")

            update(f"Writing {valid_count:,} TIN points & faces...")
            f.write(
                f'  <Surfaces>\n    <Surface name="{surf_name}">\n      <Definition surfType="TIN">\n        <Pnts>\n'
            )
            for pid, y, x, z in zip(v_ids, v_y, v_x, v_z):
                f.write(
                    f'          <P id="{pid}">{fmt(y)} {fmt(x)} {fmt(z)}</P>\n'
                )
            f.write("        </Pnts>\n        <Faces>\n")

            p1, p2 = point_ids[:-1, :-1], point_ids[:-1, 1:]
            p3, p4 = point_ids[1:, :-1], point_ids[1:, 1:]
            t1 = (p1 > 0) & (p2 > 0) & (p3 > 0)
            for a, b, c in zip(p1[t1], p2[t1], p3[t1]):
                f.write(f"          <F>{a} {b} {c}</F>\n")
            t2 = (p2 > 0) & (p4 > 0) & (p3 > 0)
            for a, b, c in zip(p2[t2], p4[t2], p3[t2]):
                f.write(f"          <F>{a} {b} {c}</F>\n")

            f.write(
                "        </Faces>\n      </Definition>\n    </Surface>\n  </Surfaces>\n</LandXML>\n"
            )
