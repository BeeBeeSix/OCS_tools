import json
import uuid
import numpy as np
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

from contour_builder import build_contour_entities, get_xy
from contour_config import OCS_BIN, parse_mcp_json
from tin_surface_reader import extract_tin_mesh


async def fetch_sessions_and_tin():
    server_params = StdioServerParameters(command=OCS_BIN, args=["--mcp"])
    async with stdio_client(server_params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()
            resp = await session.call_tool("ocs_sessions", arguments={})
            sess_list = parse_mcp_json(resp)
            new_map = {}
            for s in sess_list:
                sid = s.get("session_id", "")
                docs = s.get("documents", [])
                active_doc = next((d for d in docs if d.get("id") == s.get("document_id")), docs[-1] if docs else {})
                doc_name = active_doc.get("path") or active_doc.get("title") or "Untitled"
                new_map[f"{doc_name} [{sid[:8]}]"] = sid

            tin_handle, stats_txt = "", "No AECC_TIN_Surface detected"
            if new_map:
                first_sid = next(iter(new_map.values()))
                ext_resp = await session.call_tool(
                    "ocs_query", {"ocs_session_id": first_sid, "type": "Extended Entity", "detail": "summary"}
                )
                ext_ents = parse_mcp_json(ext_resp).get("entities", [])
                if ext_ents:
                    tin_handle = str(ext_ents[-1].get("handle", ""))
                if tin_handle:
                    st_resp = await session.call_tool(
                        "ocs_read",
                        {"ocs_session_id": first_sid, "op": "civil_surface_stats", "parameters": {"handle": tin_handle}},
                    )
                    st = parse_mcp_json(st_resp)
                    if st.get("ok"):
                        stats_txt = (
                            f"TIN {tin_handle}: {st.get('vertices')} pts, "
                            f"{st.get('visible_triangles')} tris ({st.get('min_elevation'):.1f}m–{st.get('max_elevation'):.1f}m)"
                        )
            return new_map, tin_handle, stats_txt


async def fetch_selected_handle(session_id: str):
    server_params = StdioServerParameters(command=OCS_BIN, args=["--mcp"])
    async with stdio_client(server_params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()
            insp = parse_mcp_json(await session.call_tool("ocs_inspect", {"ocs_session_id": session_id}))
            sel = insp.get("selection", [])
            return str(next(iter(sel))) if sel else None


async def clear_contour_layers(session, session_id: str, target_layers: list[str], protected_handle: str, log):
    protected = protected_handle.strip().upper()
    handles_to_delete = []
    for lyr in set(target_layers):
        if not lyr:
            continue
        for allowed_type in ("Polyline", "Text"):
            q_resp = await session.call_tool(
                "ocs_query",
                arguments={"ocs_session_id": session_id, "layer": lyr, "type": allowed_type, "limit": 10000},
            )
            q_data = parse_mcp_json(q_resp)
            ents = q_data if isinstance(q_data, list) else q_data.get("entities", [])
            for e in ents:
                h = str(e.get("handle", ""))
                if h and h.upper() != protected and e.get("type") in ("Polyline", "LwPolyline", "Text"):
                    handles_to_delete.append(h)

    if handles_to_delete:
        log(f"Deleting {len(handles_to_delete)} old contour/label entities (TIN surface protected)...")
        for i in range(0, len(handles_to_delete), 1000):
            await session.call_tool(
                "ocs_execute",
                arguments={
                    "ocs_session_id": session_id,
                    "request": {"op": "entities_delete", "request_id": str(uuid.uuid4()), "handles": handles_to_delete[i : i + 1000]},
                },
            )


async def generate_tin_contours(session_id: str, cfg: dict, log):
    server_params = StdioServerParameters(command=OCS_BIN, args=["--mcp"])
    async with stdio_client(server_params) as (read, write):
        async with ClientSession(read, write) as session:
            await session.initialize()

            tin_handle = cfg["tin_handle"]
            if cfg["replace_existing"]:
                await clear_contour_layers(
                    session, session_id, [cfg["maj_layer"], cfg["min_layer"], cfg["lbl_layer"]], tin_handle, log
                )

            bnd_handle = cfg["bnd_handle"]
            # pts, tri_arr = await extract_tin_mesh(session, session_id, tin_handle, log, bnd_handle)
            pts, tri_arr = await extract_tin_mesh(
                session, session_id, tin_handle, log, bnd_handle, cfg.get("xml_path", "")
            )

            log(f"pts {len(pts)}  X {pts[:,0].min():.1f}..{pts[:,0].max():.1f}  Y {pts[:,1].min():.1f}..{pts[:,1].max():.1f}")

            bnd_xy = None
            if bnd_handle:
                bnd_resp = await session.call_tool(
                    "ocs_query",
                    {"ocs_session_id": session_id, "handles": [bnd_handle], "detail": "geometry"},
                )
                bnd_ents = parse_mcp_json(bnd_resp).get("entities", [])
                if bnd_ents:
                    first_bnd = next(iter(bnd_ents))
                    raw_verts = first_bnd.get("vertices", [])
                    if len(raw_verts) >= 3:
                        bnd_xy = np.asarray([get_xy(v) for v in raw_verts], dtype=float)

            txt_schema_resp = await session.call_tool(
                "ocs_read",
                {"ocs_session_id": session_id, "op": "entity_schema", "parameters": {"type": "Text"}},
            )
            txt_props = parse_mcp_json(txt_schema_resp).get("schema", {}).get("properties", {})
            rot_key = next((k for k in ("rotation_deg", "angle_deg", "angle", "rotation") if k in txt_props), None)

            entities = build_contour_entities(pts, tri_arr, bnd_xy, cfg, rot_key, log)
            if not entities:
                log("Nothing to draw.")
                return

            lift = cfg.get("lift_z", True)
            zs = [ent.pop("_z", 0.0) for ent in entities]  # keep the schema-clean entities, remember each level

            MAX_BATCH_BYTES = 450_000
            batches, current_batch, current_bytes = [], [], 200
            for ent, z in zip(entities, zs):
                ent_bytes = len(json.dumps(ent, separators=(",", ":")).encode("utf-8")) + 2
                if current_batch and (current_bytes + ent_bytes > MAX_BATCH_BYTES):
                    batches.append(current_batch)
                    current_batch = [(ent, z)]
                    current_bytes = 200 + ent_bytes
                else:
                    current_batch.append((ent, z))
                    current_bytes += ent_bytes
            if current_batch:
                batches.append(current_batch)

            lifted = 0
            for idx, batch in enumerate(batches, start=1):
                chunk = [e for e, _ in batch]
                exec_resp = await session.call_tool(
                    "ocs_execute",
                    arguments={
                        "ocs_session_id": session_id,
                        "request": {"op": "entities_create", "request_id": str(uuid.uuid4()), "entities": chunk},
                    },
                )
                res = parse_mcp_json(exec_resp)
                if not res.get("ok", True):
                    raise RuntimeError(f"Batch {idx}/{len(batches)} failed: {res}")
                log(f"Sent batch {idx}/{len(batches)} ({len(chunk)} entities)...")

                if lift:
                    handles = (res.get("result") or {}).get("handles") or []
                    if len(handles) != len(batch):
                        log(f"  WARNING: got {len(handles)} handles for {len(batch)} entities; elevation lift skipped for this batch.")
                        continue
                    by_z = {}
                    for h, (_, z) in zip(handles, batch):
                        if abs(z) > 1e-9:
                            by_z.setdefault(z, []).append(h)
                    for z, hs in by_z.items():
                        for i in range(0, len(hs), 1000):
                            t_resp = parse_mcp_json(
                                await session.call_tool(
                                    "ocs_execute",
                                    arguments={
                                        "ocs_session_id": session_id,
                                        "request": {
                                            "op": "entities_transform",
                                            "request_id": str(uuid.uuid4()),
                                            "handles": hs[i : i + 1000],
                                            "action": "move",
                                            "vector": [0, 0, z],
                                        },
                                    },
                                )
                            )
                            if not t_resp.get("ok", True):
                                raise RuntimeError(f"Elevation lift to Z={z} failed: {t_resp}")
                            lifted += len(hs[i : i + 1000])
            if lift:
                log(f"Lifted {lifted} entities to their true elevation.")

            log("Success! AECC_TIN_Surface contours & labels are now live in Open CAD Studio.")
