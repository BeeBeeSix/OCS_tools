import json
import traceback
from typing import Callable, Dict, List, Optional
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

from ocs_helpers import (
    find_closed_boundary_handle,
    log_surface_stats,
    parse_mcp_json,
    req_id,
)


async def build_surface_in_ocs(
    ocs_bin: str,
    surface: Dict,
    dwg_out: str,
    swap_xy: bool,
    use_closed_boundary: bool,
    delete_temp_points: bool,
    rename_surface: bool,
    max_triangle_length: Optional[float],
    log: Callable[[str], None],
    set_progress: Callable[[float], None],
) -> None:
    try:
        log("Connecting to Open CAD Studio via MCP...")
        async with stdio_client(StdioServerParameters(command=ocs_bin, args=["--mcp"])) as (read, write):
            async with ClientSession(read, write) as session:
                await session.initialize()

                sessions_resp = parse_mcp_json(await session.call_tool("ocs_sessions", {}))
                if not sessions_resp:
                    raise RuntimeError("No active Open CAD Studio session found. Open a drawing first.")
                sid = next(iter(sessions_resp))["session_id"]
                log(f"Connected to session: {sid}")

                await session.call_tool(
                    "ocs_execute",
                    {"ocs_session_id": sid, "request": {"op": "cancel", "request_id": req_id("cancel")}},
                )

                boundary_handle: Optional[str] = (
                    await find_closed_boundary_handle(session, sid, log) if use_closed_boundary else None
                )

                pts = list(surface["points"].values())
                total = len(pts)
                log(f"Batch-creating {total} Point entities...")

                for idx in range(0, total, 500):
                    chunk = pts[idx : idx + 500]
                    entities_payload = [
                        {"type": "Point", "location": [(x if swap_xy else y), (y if swap_xy else x), z]}
                        for (x, y, z) in chunk
                    ]
                    create_res = parse_mcp_json(
                        await session.call_tool(
                            "ocs_execute",
                            {
                                "ocs_session_id": sid,
                                "request": {
                                    "op": "entities_create",
                                    "request_id": req_id("pts"),
                                    "entities": entities_payload,
                                },
                            },
                        )
                    )
                    if isinstance(create_res, str) or (isinstance(create_res, dict) and create_res.get("ok") is False):
                        raise RuntimeError(f"entities_create failed: {create_res}")
                    set_progress((min(idx + 500, total) / total) * 70.0)

                sel_res = parse_mcp_json(
                    await session.call_tool(
                        "ocs_execute",
                        {
                            "ocs_session_id": sid,
                            "request": {"op": "select", "request_id": req_id("sel_pts"), "clear": True, "type": "Point"},
                        },
                    )
                )
                point_handles: List[str] = (
                    (sel_res.get("state") or {}).get("selection", []) if isinstance(sel_res, dict) else []
                )
                log(f"Selected {len(point_handles)} Point handles.")
                if not point_handles:
                    raise RuntimeError("No Point entities found in active document after creation.")

                log("Starting CREATESURFACE...")
                step_res = parse_mcp_json(
                    await session.call_tool(
                        "ocs_execute",
                        {"ocs_session_id": sid, "request": {"op": "start", "request_id": req_id("start"), "cmd": "CREATESURFACE"}},
                    )
                )

                step_count, sent_points, sent_boundary = 0, False, False
                while isinstance(step_res, dict) and step_res.get("status") == "waiting_input" and step_count < 12:
                    step_count += 1
                    cmd_info = (step_res.get("state") or {}).get("command") or {}
                    accepts = cmd_info.get("accepts", [])
                    prompt_lower = cmd_info.get("prompt", "").lower()

                    if "selection" in accepts and not sent_points and ("point" in prompt_lower or step_count == 1):
                        sent_points = True
                        inp = {"op": "input", "request_id": req_id("in_pts"), "kind": "selection", "handles": point_handles}
                    elif "selection" in accepts and "boundary" in prompt_lower and boundary_handle and not sent_boundary:
                        sent_boundary = True
                        log(f"Applying outer boundary polyline {boundary_handle}...")
                        inp = {"op": "input", "request_id": req_id("in_bnd"), "kind": "selection", "handles": [boundary_handle]}
                    elif "enter" in accepts:
                        inp = {"op": "input", "request_id": req_id("in_enter"), "kind": "enter"}
                    elif "text" in accepts or cmd_info.get("free_text"):
                        inp = {"op": "input", "request_id": req_id("in_txt"), "kind": "text", "text": surface["name"]}
                    else:
                        break

                    step_res = parse_mcp_json(
                        await session.call_tool("ocs_execute", {"ocs_session_id": sid, "request": inp})
                    )

                set_progress(85.0)
                await log_surface_stats(session, sid, log)

                if delete_temp_points and point_handles:
                    log(f"Deleting {len(point_handles)} temporary Point entities...")
                    await session.call_tool(
                        "ocs_execute",
                        {
                            "ocs_session_id": sid,
                            "request": {"op": "entities_delete", "request_id": req_id("del_pts"), "handles": point_handles},
                        },
                    )

                if dwg_out:
                    log(f"Saving DWG to {dwg_out}...")
                    await session.call_tool(
                        "ocs_execute",
                        {"ocs_session_id": sid, "request": {"op": "save", "request_id": req_id("save"), "path": dwg_out}},
                    )
                    log("DWG saved.")

                set_progress(100.0)
                log(f"SUCCESS! Built AECC_TIN_Surface '{surface['name']}'.")

    except BaseException:
        log(traceback.format_exc())
        raise
