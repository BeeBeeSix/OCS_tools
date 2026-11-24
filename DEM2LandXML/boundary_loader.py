import json
from pathlib import Path
from pyproj import CRS, Transformer
import shapefile


def load_boundary_polygon(file_path, target_crs):
    ext = Path(file_path).suffix.lower()
    coords, src_crs = [], None

    if ext in (".geojson", ".json"):
        with open(file_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        geom = (
            data["features"][0]["geometry"]
            if data.get("type") == "FeatureCollection"
            else data.get("geometry", data)
        )
        ring = (
            geom["coordinates"][0]
            if geom["type"] == "Polygon"
            else geom["coordinates"][0][0]
        )
        coords = [(pt[0], pt[1]) for pt in ring]
        src_crs = CRS.from_epsg(4326)

    elif ext == ".shp":
        sf = shapefile.Reader(file_path)
        shapes = sf.shapes()
        if not shapes:
            raise ValueError("Shapefile contains no shapes.")
        pts = shapes[0].points
        parts = list(shapes[0].parts) + [len(pts)]
        coords = [(pts[i][0], pts[i][1]) for i in range(parts[0], parts[1])]

        prj_path = Path(file_path).with_suffix(".prj")
        if prj_path.exists():
            src_crs = CRS.from_wkt(prj_path.read_text(encoding="utf-8"))

    else:
        raise ValueError("Unsupported file type. Use .shp or .geojson")

    if src_crs and target_crs and src_crs != CRS.from_user_input(target_crs):
        tf = Transformer.from_crs(src_crs, target_crs, always_xy=True)
        coords = [tf.transform(x, y) for x, y in coords]

    return coords
