import json
import uuid
from typing import Callable, Optional
from mcp import ClientSession


def parse_mcp_json(resp):
    text = next(iter(resp.content)).text
    try:
        return json.loads(text)
    except Exception:
        return text


def req_id(prefix: str = "req") -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"


def is_polyline_closed(poly: dict) -> bool:
    if poly.get("closed") is True or poly.get("is_closed") is True:
        return True
    flags = poly.get("flags")
    if isinstance(flags, int) and (flags & 1):
        return True
    # Check nested properties (e.g. properties.misc.closed or properties.geometry.closed)
    text_dump = json.dumps(poly.get("properties") or {}).lower()
    if '"closed": true' in text_dump or '"closed": "yes"' in text_dump or '"is_closed": true' in text_dump:
        return True
    verts = poly.get("vertices") or []
    if len(verts) >= 3:
        first_v = next(iter(verts))
        last_v = next(iter(reversed(verts)))
        if isinstance(first_v, dict) and isinstance(last_v, dict):
            dx = float(first_v.get("x", 0.0)) - float(last_v.get("x", 0.0))
            dy = float(first_v.get("y", 0.0)) - float(last_v.get("y", 0.0))
            return abs(dx) < 1e-2 and abs(dy) < 1e-2
        if isinstance(first_v, (list, tuple)) and isinstance(last_v, (list, tuple)):
            c0, cn = list(first_v), list(last_v)
            if len(c0) >= 2 and len(cn) >= 2:
                dx = float(c0.pop(0)) - float(cn.pop(0))
                dy = float(c0.pop(0)) - float(cn.pop(0))
                return abs(dx) < 1e-2 and abs(dy) < 1e-2
    return False


async def find_closed_boundary_handle(session: ClientSession, sid: str, log: Callable[[str], None]) -> Optional[str]:
    q_resp = parse_mcp_json(
        await session.call_tool("ocs_query", {"ocs_session_id": sid, "type": "Polyline", "detail": "full"})
    )
    polys = q_resp.get("entities", []) if isinstance(q_resp, dict) else []
    candidates = []
    for p in polys:
        verts = p.get("vertices") or []
        closed = is_polyline_closed(p)
        log(f"Inspected Polyline handle={p.get('handle')}: vertices={len(verts)}, closed={closed}")
        if closed and p.get("handle"):
            candidates.append(p)
    if not candidates:
        log("No closed Polyline found in active document for outer boundary.")
        return None
    best = max(candidates, key=lambda p: len(p.get("vertices") or []))
    handle = str(best.get("handle"))
    log(f"Selected outer boundary handle={handle} (vertices={len(best.get('vertices') or [])})")
    return handle


async def log_surface_stats(session: ClientSession, sid: str, log: Callable[[str], None]) -> None:
    q_surf = parse_mcp_json(
        await session.call_tool("ocs_query", {"ocs_session_id": sid, "type": "Extended Entity", "detail": "summary"})
    )
    ents = q_surf.get("entities", []) if isinstance(q_surf, dict) else []
    if not ents:
        return
    last_ent = next(iter(reversed(ents)))
    surf_handle = str(last_ent.get("handle"))
    stats_res = parse_mcp_json(
        await session.call_tool(
            "ocs_read",
            {"ocs_session_id": sid, "op": "civil_surface_stats", "parameters": {"handle": surf_handle}},
        )
    )
    log(f"AECC_TIN_Surface ({surf_handle}) stats: {json.dumps(stats_res)}")
