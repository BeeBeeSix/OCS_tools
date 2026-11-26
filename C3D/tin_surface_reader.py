import json
import uuid
import numpy as np
from contour_config import parse_mcp_json
from landxml_parser import parse_landxml_surfaces


def load_mesh_from_any_landxml(xml_path: str):
    """Parse vertices and visible faces from either raw 3DFace LandXML or standard LandXML 1.2."""
    surfaces = parse_landxml_surfaces(xml_path)
    if not surfaces:
        raise RuntimeError(f"No <Surface> elements found in '{xml_path}'.")
    surf = next(iter(surfaces))
    pts_dict = surf.get("points", {})
    faces = surf.get("faces", [])

    pid_to_idx, verts, tris = {}, [], []
    for pid, (n, e, z) in pts_dict.items():
        pid_to_idx[pid] = len(verts)
        verts.append([float(e), float(n), float(z)])  # LandXML is N,E,Z -> X=E, Y=N

    for f in faces:
        ids = list(f)
        if len(ids) >= 3 and all(i in pid_to_idx for i in ids[:3]):
            tris.append([pid_to_idx[ids.pop(0)], pid_to_idx[ids.pop(0)], pid_to_idx[ids.pop(0)]])

    if len(verts) < 3:
        raise RuntimeError(f"No valid vertices found in '{xml_path}'.")
    return np.asarray(verts, dtype=float), (np.asarray(tris, dtype=int) if tris else None)


async def extract_tin_mesh(session, session_id: str, tin_handle: str, log, bnd_handle: str = "", xml_path: str = ""):
    """Read TIN mesh from AECC_TIN_Surface XData cache, or load from LandXML and cache onto AECC_TIN_Surface."""
    # Native read (needs the civil_surface_mesh op in OCS): no LandXML, works for TINs built in OCS.
    native = parse_mcp_json(
        await session.call_tool(
            "ocs_read",
            {"ocs_session_id": session_id, "op": "civil_surface_mesh", "parameters": {"handle": tin_handle}},
        )
    )
    if isinstance(native, dict) and native.get("ok") and native.get("pts"):
        pts = np.asarray(native["pts"], dtype=float)
        tris = np.asarray(native["tris"], dtype=int) if native.get("tris") else None
        log(f"Read TIN mesh natively from '{tin_handle}': {len(pts)} pts, {0 if tris is None else len(tris)} visible tris.")
        return pts, tris
    log("Native civil_surface_mesh op unavailable; using cache / LandXML.")

    chunks = []
    xd_res = parse_mcp_json(
        await session.call_tool(
            "ocs_read",
            {"ocs_session_id": session_id, "op": "xdata_get", "parameters": {"handle": tin_handle, "app": "OCS_TIN_MESH_EN"}},
        )
    )
    if isinstance(xd_res, dict) and xd_res.get("ok"):
        entries = xd_res.get("data") or []
        chunks = [str(item.get("value", "")) for item in entries if isinstance(item, dict) and item.get("code") == 1000]

    if not chunks:  # fallback: read the XData straight from the entity record
        rec = parse_mcp_json(
            await session.call_tool(
                "ocs_read",
                {"ocs_session_id": session_id, "op": "records", "parameters": {"collection": "entities", "handle": tin_handle}},
            )
        )
        for r in (rec.get("records") or []) if isinstance(rec, dict) else []:
            if str(r.get("handle", "")).upper() != tin_handle.upper():
                continue
            xrecs = r.get("properties", {}).get("common", {}).get("extended_data", {}).get("records", [])
            for xr in xrecs:
                if xr.get("application_name") == "OCS_TIN_MESH_EN":
                    chunks = [v["String"] for v in xr.get("values", []) if isinstance(v, dict) and "String" in v]

    if chunks:
        try:
            payload = json.loads("".join(chunks))
            pts = np.asarray(payload["pts"], dtype=float)
            tris = np.asarray(payload["tris"], dtype=int) if payload.get("tris") else None
            log(f"Loaded cached TIN mesh from AECC_TIN_Surface '{tin_handle}' ({len(pts)} pts).")
            return pts, tris
        except Exception as exc:
            log(f"Cached TIN mesh unreadable ({type(exc).__name__}: {exc}); falling back to LandXML.")
    else:
        log("No cached TIN mesh found on the surface.")

    if not xml_path:
        raise RuntimeError("Click 'Browse...' next to LandXML Source and select your LandXML file (e.g. BD37-LXML.xml).")

    pts, tris = load_mesh_from_any_landxml(xml_path)
    log(f"Loaded {len(pts)} vertices and {len(tris) if tris is not None else 0} faces from '{xml_path}'.")

    raw_json = json.dumps(
        {"pts": np.round(pts, 4).tolist(), "tris": tris.tolist() if tris is not None else []},
        separators=(",", ":"),
    )
    xd_entries = [{"code": 1000, "value": raw_json[i : i + 250]} for i in range(0, len(raw_json), 250)]
    await session.call_tool(
        "ocs_execute",
        {
            "ocs_session_id": session_id,
            "request": {
                "op": "xdata_set",
                "request_id": str(uuid.uuid4()),
                "app": "OCS_TIN_MESH_EN",
                "handles": [tin_handle],
                "data": xd_entries,
            },
        },
    )
    log(f"Cached TIN mesh onto AECC_TIN_Surface '{tin_handle}' XData for future runs.")
    return pts, tris
