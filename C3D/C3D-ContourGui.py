import asyncio
import json
import math
import threading
import uuid
import tkinter as tk
from tkinter import ttk, messagebox

import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.tri as mtri
from matplotlib.path import Path as MplPath

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

OCS_BIN = r"C:\Program Files\Open CAD Studio\OpenCADStudio.exe"

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
    closed = np.allclose(pts, pts[-1], atol=1e-6)
    for _ in range(iterations):
        if len(pts) < 3:
            break
        q = 0.75 * pts[:-1] + 0.25 * pts[1:]
        r = 0.25 * pts[:-1] + 0.75 * pts[1:]
        new_pts = np.empty((len(q) + len(r), 2), dtype=float)
        new_pts[0::2] = q
        new_pts[1::2] = r
        if not closed:
            pts = np.vstack([pts, new_pts, pts[-1]])
        else:
            pts = np.vstack([new_pts, new_pts])
    return pts


class ContourGeneratorGUI:
    def __init__(self, root: tk.Tk):
        self.root = root
        self.root.title("Open CAD Studio — Civil TIN Contour Styler")
        self.root.geometry("660x810")
        self.root.resizable(False, False)

        self.sessions_map = {}

        # Connection & TIN Surface variables
        self.session_var = tk.StringVar(value="Click 'Refresh' to connect...")
        self.surface_handle_var = tk.StringVar(value="268E")
        self.surface_info_var = tk.StringVar(value="Surface stats: click Refresh to inspect AECC_TIN_SURFACE")
        self.source_layer_var = tk.StringVar(value="TER-PNT")
        self.boundary_handle_var = tk.StringVar(value="24B8")
        self.max_edge_var = tk.DoubleVar(value=120.0)

        # Interval & smoothing variables
        self.minor_int_var = tk.DoubleVar(value=1.0)
        self.major_int_var = tk.DoubleVar(value=5.0)
        self.smooth_var = tk.IntVar(value=1)

        # Major style variables
        self.maj_layer_var = tk.StringVar(value="C-TOPO-MAJR")
        self.maj_color_preset_var = tk.StringVar(value="1 - Red")
        self.maj_aci_var = tk.IntVar(value=1)
        self.maj_lw_var = tk.StringVar(value="0.35 mm (35)")
        self.maj_width_var = tk.DoubleVar(value=0.4)

        # Minor style variables
        self.min_layer_var = tk.StringVar(value="C-TOPO-MINR")
        self.min_color_preset_var = tk.StringVar(value="8 - Dark Gray")
        self.min_aci_var = tk.IntVar(value=8)
        self.min_lw_var = tk.StringVar(value="0.13 mm (13)")
        self.min_width_var = tk.DoubleVar(value=0.0)

        # Label variables
        self.add_labels_var = tk.BooleanVar(value=True)
        self.lbl_layer_var = tk.StringVar(value="C-TOPO-TEXT")
        self.lbl_height_var = tk.DoubleVar(value=2.5)
        self.lbl_aci_var = tk.IntVar(value=7)
        self.lbl_min_len_var = tk.DoubleVar(value=40.0)
        self.lbl_decimals_var = tk.IntVar(value=0)

        self.replace_existing_var = tk.BooleanVar(value=True)

        self._build_ui()
        self.refresh_sessions()
    def _build_ui(self):
        pad = {"padx": 10, "pady": 4}

        # 1. Connection & Civil TIN Surface Frame
        conn_frame = ttk.LabelFrame(self.root, text="1. Live Session & Civil TIN Surface", padding=8)
        conn_frame.pack(fill="x", **pad)

        ttk.Label(conn_frame, text="Active Session:").grid(row=0, column=0, sticky="w")
        self.session_combo = ttk.Combobox(conn_frame, textvariable=self.session_var, state="readonly", width=54)
        self.session_combo.grid(row=0, column=1, columnspan=3, padx=6, sticky="w")
        ttk.Button(conn_frame, text="Refresh", command=self.refresh_sessions).grid(row=0, column=4)

        ttk.Label(conn_frame, text="TIN Handle:").grid(row=1, column=0, sticky="w", pady=(6, 0))
        ttk.Entry(conn_frame, textvariable=self.surface_handle_var, width=10).grid(row=1, column=1, sticky="w", padx=6, pady=(6, 0))
        ttk.Label(conn_frame, textvariable=self.surface_info_var, foreground="#1E5AA8").grid(row=1, column=2, columnspan=3, sticky="w", pady=(6, 0))

        ttk.Label(conn_frame, text="Point Layer:").grid(row=2, column=0, sticky="w", pady=(6, 0))
        ttk.Entry(conn_frame, textvariable=self.source_layer_var, width=14).grid(row=2, column=1, sticky="w", padx=6, pady=(6, 0))

        ttk.Label(conn_frame, text="Boundary Handle:").grid(row=2, column=2, sticky="w", pady=(6, 0))
        b_row = ttk.Frame(conn_frame)
        b_row.grid(row=2, column=3, columnspan=2, sticky="w", pady=(6, 0))
        ttk.Entry(b_row, textvariable=self.boundary_handle_var, width=10).pack(side="left", padx=(4, 6))
        ttk.Button(b_row, text="Pick Selected", command=self.pick_selected_boundary).pack(side="left")

        # 2. Intervals & Geometry Frame
        int_frame = ttk.LabelFrame(self.root, text="2. Contour Intervals & TIN Clipping", padding=8)
        int_frame.pack(fill="x", **pad)

        ttk.Label(int_frame, text="Minor (m):").grid(row=0, column=0, sticky="w")
        ttk.Entry(int_frame, textvariable=self.minor_int_var, width=8).grid(row=0, column=1, sticky="w", padx=(4, 12))

        ttk.Label(int_frame, text="Major (m):").grid(row=0, column=2, sticky="w")
        ttk.Entry(int_frame, textvariable=self.major_int_var, width=8).grid(row=0, column=3, sticky="w", padx=(4, 12))

        ttk.Label(int_frame, text="Smooth:").grid(row=0, column=4, sticky="w")
        ttk.Spinbox(int_frame, from_=0, to=3, textvariable=self.smooth_var, width=4).grid(row=0, column=5, sticky="w", padx=(4, 12))

        ttk.Label(int_frame, text="Max Edge (m):").grid(row=0, column=6, sticky="w")
        ttk.Entry(int_frame, textvariable=self.max_edge_var, width=8).grid(row=0, column=7, sticky="w", padx=4)

        # 3. Major Contours Style
        maj_frame = ttk.LabelFrame(self.root, text="3. Major Contours Styling", padding=8)
        maj_frame.pack(fill="x", **pad)

        ttk.Label(maj_frame, text="Layer Name:").grid(row=0, column=0, sticky="w")
        ttk.Entry(maj_frame, textvariable=self.maj_layer_var, width=22).grid(row=0, column=1, sticky="w", padx=6)

        ttk.Label(maj_frame, text="Color Preset:").grid(row=0, column=2, sticky="w")
        maj_cb = ttk.Combobox(maj_frame, textvariable=self.maj_color_preset_var, values=list(ACI_PRESETS.keys()), state="readonly", width=16)
        maj_cb.grid(row=0, column=3, sticky="w", padx=4)
        maj_cb.bind("<<ComboboxSelected>>", lambda e: self._sync_color_preset("maj"))

        ttk.Label(maj_frame, text="ACI #:").grid(row=0, column=4, sticky="w", padx=(6, 0))
        ttk.Entry(maj_frame, textvariable=self.maj_aci_var, width=5).grid(row=0, column=5, sticky="w", padx=4)
        self.maj_swatch = tk.Label(maj_frame, text="   ", bg="#FF0000", relief="solid", borderwidth=1)
        self.maj_swatch.grid(row=0, column=6, padx=4)

        ttk.Label(maj_frame, text="Lineweight:").grid(row=1, column=0, sticky="w", pady=(6, 0))
        ttk.Combobox(maj_frame, textvariable=self.maj_lw_var, values=list(LINEWEIGHT_OPTIONS.keys()), state="readonly", width=19).grid(row=1, column=1, sticky="w", padx=6, pady=(6, 0))

        ttk.Label(maj_frame, text="Constant Width:").grid(row=1, column=2, sticky="w", pady=(6, 0))
        ttk.Entry(maj_frame, textvariable=self.maj_width_var, width=10).grid(row=1, column=3, sticky="w", padx=4, pady=(6, 0))

        # 4. Minor Contours Style
        min_frame = ttk.LabelFrame(self.root, text="4. Minor Contours Styling", padding=8)
        min_frame.pack(fill="x", **pad)

        ttk.Label(min_frame, text="Layer Name:").grid(row=0, column=0, sticky="w")
        ttk.Entry(min_frame, textvariable=self.min_layer_var, width=22).grid(row=0, column=1, sticky="w", padx=6)

        ttk.Label(min_frame, text="Color Preset:").grid(row=0, column=2, sticky="w")
        min_cb = ttk.Combobox(min_frame, textvariable=self.min_color_preset_var, values=list(ACI_PRESETS.keys()), state="readonly", width=16)
        min_cb.grid(row=0, column=3, sticky="w", padx=4)
        min_cb.bind("<<ComboboxSelected>>", lambda e: self._sync_color_preset("min"))

        ttk.Label(min_frame, text="ACI #:").grid(row=0, column=4, sticky="w", padx=(6, 0))
        ttk.Entry(min_frame, textvariable=self.min_aci_var, width=5).grid(row=0, column=5, sticky="w", padx=4)
        self.min_swatch = tk.Label(min_frame, text="   ", bg="#808080", relief="solid", borderwidth=1)
        self.min_swatch.grid(row=0, column=6, padx=4)

        ttk.Label(min_frame, text="Lineweight:").grid(row=1, column=0, sticky="w", pady=(6, 0))
        ttk.Combobox(min_frame, textvariable=self.min_lw_var, values=list(LINEWEIGHT_OPTIONS.keys()), state="readonly", width=19).grid(row=1, column=1, sticky="w", padx=6, pady=(6, 0))

        ttk.Label(min_frame, text="Constant Width:").grid(row=1, column=2, sticky="w", pady=(6, 0))
        ttk.Entry(min_frame, textvariable=self.min_width_var, width=10).grid(row=1, column=3, sticky="w", padx=4, pady=(6, 0))

        # 5. Elevation Labels Frame
        lbl_frame = ttk.LabelFrame(self.root, text="5. Major Contour Elevation Labels", padding=8)
        lbl_frame.pack(fill="x", **pad)

        ttk.Checkbutton(lbl_frame, text="Add Elevation Labels on Major Contours", variable=self.add_labels_var).grid(row=0, column=0, columnspan=3, sticky="w")

        ttk.Label(lbl_frame, text="Label Layer:").grid(row=1, column=0, sticky="w", pady=(6, 0))
        ttk.Entry(lbl_frame, textvariable=self.lbl_layer_var, width=22).grid(row=1, column=1, sticky="w", padx=6, pady=(6, 0))

        ttk.Label(lbl_frame, text="Text Height:").grid(row=1, column=2, sticky="w", pady=(6, 0))
        ttk.Entry(lbl_frame, textvariable=self.lbl_height_var, width=8).grid(row=1, column=3, sticky="w", padx=4, pady=(6, 0))

        ttk.Label(lbl_frame, text="ACI Color:").grid(row=1, column=4, sticky="w", pady=(6, 0))
        ttk.Entry(lbl_frame, textvariable=self.lbl_aci_var, width=6).grid(row=1, column=5, sticky="w", padx=4, pady=(6, 0))

        ttk.Label(lbl_frame, text="Min Length (m):").grid(row=2, column=0, sticky="w", pady=(6, 0))
        ttk.Entry(lbl_frame, textvariable=self.lbl_min_len_var, width=10).grid(row=2, column=1, sticky="w", padx=6, pady=(6, 0))

        ttk.Label(lbl_frame, text="Decimals:").grid(row=2, column=2, sticky="w", pady=(6, 0))
        ttk.Spinbox(lbl_frame, from_=0, to=3, textvariable=self.lbl_decimals_var, width=5).grid(row=2, column=3, sticky="w", padx=4, pady=(6, 0))

        # 6. Action Buttons & Log
        act_frame = ttk.Frame(self.root, padding=(10, 4))
        act_frame.pack(fill="x")

        ttk.Checkbutton(
            act_frame,
            text="Clear existing entities on target contour/label layers first",
            variable=self.replace_existing_var,
        ).pack(anchor="w", pady=(0, 6))

        btn_row = ttk.Frame(act_frame)
        btn_row.pack(fill="x")

        self.run_btn = ttk.Button(btn_row, text="Generate & Style Contours", command=self.run_generation)
        self.run_btn.pack(side="left", padx=(0, 8), ipadx=10, ipady=4)

        self.clear_btn = ttk.Button(btn_row, text="Clear Contour Layers Only", command=self.run_clear_only)
        self.clear_btn.pack(side="left", ipadx=6, ipady=4)

        log_frame = ttk.LabelFrame(self.root, text="Status Log", padding=8)
        log_frame.pack(fill="both", expand=True, **pad)

        self.log_text = tk.Text(log_frame, height=8, wrap="word", state="disabled", font=("Consolas", 9))
        self.log_text.pack(fill="both", expand=True)
    def _sync_color_preset(self, which):
        if which == "maj":
            aci, hex_col = ACI_PRESETS[self.maj_color_preset_var.get()]
            self.maj_aci_var.set(aci)
            self.maj_swatch.configure(bg=hex_col)
        else:
            aci, hex_col = ACI_PRESETS[self.min_color_preset_var.get()]
            self.min_aci_var.set(aci)
            self.min_swatch.configure(bg=hex_col)

    def log(self, msg: str):
        def _append():
            self.log_text.configure(state="normal")
            self.log_text.insert("end", msg + "\n")
            self.log_text.see("end")
            self.log_text.configure(state="disabled")
        self.root.after(0, _append)

    def _set_busy(self, busy: bool):
        def _toggle():
            state = "disabled" if busy else "normal"
            self.run_btn.configure(state=state)
            self.clear_btn.configure(state=state)
        self.root.after(0, _toggle)

    def _run_async_in_thread(self, coro_fn):
        def _worker():
            self._set_busy(True)
            try:
                asyncio.run(coro_fn())
            except BaseException as exc:
                while hasattr(exc, "exceptions") and exc.exceptions:
                    exc = next(iter(exc.exceptions))
                err_msg = f"{type(exc).__name__}: {exc}"
                self.log(f"ERROR: {err_msg}")
                self.root.after(0, lambda msg=err_msg: messagebox.showerror("Open CAD Studio Error", msg))
            finally:
                self._set_busy(False)
        threading.Thread(target=_worker, daemon=True).start()


    def refresh_sessions(self):
        async def _fetch():
            self.log("Querying active Open CAD Studio sessions & TIN surfaces...")
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
                        label = f"{doc_name} [{sid[:8]}]"
                        new_map[label] = sid

                    tin_handle = self.surface_handle_var.get().strip()
                    stats_txt = "No AECC_TIN_SURFACE detected"
                    if new_map:
                        first_sid = next(iter(new_map.values()))
                        ext_resp = await session.call_tool(
                            "ocs_query",
                            {"ocs_session_id": first_sid, "type": "Extended Entity", "detail": "summary"},
                        )
                        ext_ents = parse_mcp_json(ext_resp).get("entities", [])
                        if ext_ents:
                            tin_handle = next(iter(ext_ents)).get("handle", tin_handle)
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

                    def _update_ui():
                        self.sessions_map = new_map
                        labels = list(new_map.keys())
                        self.session_combo["values"] = labels
                        if labels:
                            first_lbl = next(iter(labels))
                            self.session_var.set(first_lbl)
                            self.surface_handle_var.set(tin_handle)
                            self.surface_info_var.set(stats_txt)
                            self.log(f"Connected: {first_lbl} | {stats_txt}")
                        else:
                            self.session_var.set("No active GUI session found")
                            self.log("No active Open CAD Studio sessions found.")
                    self.root.after(0, _update_ui)

        self._run_async_in_thread(_fetch)

    def pick_selected_boundary(self):
        session_id = self.sessions_map.get(self.session_var.get())
        if not session_id:
            messagebox.showwarning("No Session", "Select an active Open CAD Studio session first.")
            return

        async def _pick():
            server_params = StdioServerParameters(command=OCS_BIN, args=["--mcp"])
            async with stdio_client(server_params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()
                    insp = parse_mcp_json(await session.call_tool("ocs_inspect", {"ocs_session_id": session_id}))
                    sel = insp.get("selection", [])
                    if not sel:
                        self.log("Select a boundary polyline in Open CAD Studio first, then click 'Pick Selected'.")
                        return
                    chosen = next(iter(sel))
                    self.root.after(0, lambda: self.boundary_handle_var.set(chosen))
                    self.log(f"Boundary handle set to selected entity: {chosen}")

        self._run_async_in_thread(_pick)

    async def _clear_layers(self, session, session_id: str, target_layers: list[str]):
        protected_handle = self.surface_handle_var.get().strip().upper()
        handles_to_delete = []
        for lyr in set(target_layers):
            if not lyr:
                continue
            for allowed_type in ("Polyline", "Text"):
                q_resp = await session.call_tool(
                    "ocs_query",
                    arguments={
                        "ocs_session_id": session_id,
                        "layer": lyr,
                        "type": allowed_type,
                        "limit": 10000,
                    },
                )
                q_data = parse_mcp_json(q_resp)
                ents = q_data if isinstance(q_data, list) else q_data.get("entities", [])
                for e in ents:
                    h = str(e.get("handle", ""))
                    if h and h.upper() != protected_handle and e.get("type") in ("Polyline", "LwPolyline", "Text"):
                        handles_to_delete.append(h)

        if handles_to_delete:
            self.log(f"Deleting {len(handles_to_delete)} old contour/label entities (TIN surface protected)...")
            for i in range(0, len(handles_to_delete), 1000):
                chunk = handles_to_delete[i : i + 1000]
                await session.call_tool(
                    "ocs_execute",
                    arguments={
                        "ocs_session_id": session_id,
                        "request": {
                            "op": "entities_delete",
                            "request_id": str(uuid.uuid4()),
                            "handles": chunk,
                        },
                    },
                )

    def run_clear_only(self):
        session_id = self.sessions_map.get(self.session_var.get())
        if not session_id:
            messagebox.showwarning("No Session", "Select an active Open CAD Studio session first.")
            return

        layers = [self.maj_layer_var.get().strip(), self.min_layer_var.get().strip(), self.lbl_layer_var.get().strip()]

        async def _clear():
            server_params = StdioServerParameters(command=OCS_BIN, args=["--mcp"])
            async with stdio_client(server_params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()
                    await self._clear_layers(session, session_id, layers)
                    self.log("Target layers cleared.")

        self._run_async_in_thread(_clear)

    def run_generation(self):
        session_id = self.sessions_map.get(self.session_var.get())
        if not session_id:
            messagebox.showwarning("No Session", "Select an active Open CAD Studio session first.")
            return

        src_layer = self.source_layer_var.get().strip()
        bnd_handle = self.boundary_handle_var.get().strip()
        max_edge = float(self.max_edge_var.get())
        minor_int = float(self.minor_int_var.get())
        major_int = float(self.major_int_var.get())
        smooth_passes = int(self.smooth_var.get())

        maj_layer = self.maj_layer_var.get().strip() or "C-TOPO-MAJR"
        maj_color = int(self.maj_aci_var.get())
        maj_lw = LINEWEIGHT_OPTIONS[self.maj_lw_var.get()]
        maj_width = float(self.maj_width_var.get())

        min_layer = self.min_layer_var.get().strip() or "C-TOPO-MINR"
        min_color = int(self.min_aci_var.get())
        min_lw = LINEWEIGHT_OPTIONS[self.min_lw_var.get()]
        min_width = float(self.min_width_var.get())

        add_labels = bool(self.add_labels_var.get())
        lbl_layer = self.lbl_layer_var.get().strip() or "C-TOPO-TEXT"
        lbl_height = float(self.lbl_height_var.get())
        lbl_color = int(self.lbl_aci_var.get())
        lbl_min_len = float(self.lbl_min_len_var.get())
        lbl_decimals = int(self.lbl_decimals_var.get())

        replace_existing = bool(self.replace_existing_var.get())

        if minor_int <= 0 or major_int <= 0:
            messagebox.showerror("Invalid Interval", "Contour intervals must be greater than 0.")
            return

        async def _generate():
            from operator import itemgetter
            get_xy = itemgetter(0, 1)

            server_params = StdioServerParameters(command=OCS_BIN, args=["--mcp"])
            async with stdio_client(server_params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()

                    if replace_existing:
                        await self._clear_layers(session, session_id, [maj_layer, min_layer, lbl_layer])

                    self.log(f"Reading 3D points from layer '{src_layer}'...")
                    pts_resp = await session.call_tool(
                        "ocs_query",
                        arguments={"ocs_session_id": session_id, "layer": src_layer, "type": "Point", "limit": 10000},
                    )
                    pts_data = parse_mcp_json(pts_resp)
                    ent_list = pts_data if isinstance(pts_data, list) else pts_data.get("entities", [])
                    coords = [e["location"] for e in ent_list if "location" in e and len(e["location"]) >= 3]

                    if len(coords) < 3:
                        raise RuntimeError(f"Found only {len(coords)} 3D points on layer '{src_layer}'. Need at least 3.")

                    pts = np.asarray(coords, dtype=float)
                    triang = mtri.Triangulation(pts[:, 0], pts[:, 1])
                    z_vals = pts[:, 2]

                    tris = triang.triangles
                    mask = np.zeros(len(tris), dtype=bool)

                    if max_edge > 0:
                        p0, p1, p2 = pts[tris[:, 0], :2], pts[tris[:, 1], :2], pts[tris[:, 2], :2]
                        e01 = np.hypot(p1[:, 0] - p0[:, 0], p1[:, 1] - p0[:, 1])
                        e12 = np.hypot(p2[:, 0] - p1[:, 0], p2[:, 1] - p1[:, 1])
                        e20 = np.hypot(p0[:, 0] - p2[:, 0], p0[:, 1] - p2[:, 1])
                        mask |= (e01 > max_edge) | (e12 > max_edge) | (e20 > max_edge)

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
                                centroids = pts[tris, :2].mean(axis=1)
                                inside = MplPath(bnd_xy).contains_points(centroids)
                                mask |= ~inside
                                self.log(f"Clipped TIN to boundary '{bnd_handle}' ({int(np.sum(~mask))} active triangles).")

                    if np.any(mask):
                        triang.set_mask(mask)

                    z_min = math.floor(z_vals.min() / minor_int) * minor_int
                    z_max = math.ceil(z_vals.max() / minor_int) * minor_int
                    levels = np.arange(z_min, z_max + minor_int * 0.5, minor_int)

                    fig, ax = plt.subplots()
                    cs = ax.tricontour(triang, z_vals, levels=levels)

                    entities_to_create = []
                    major_count, minor_count, label_count = 0, 0, 0
                    # added
                    txt_schema_resp = await session.call_tool(
                        "ocs_read",
                        {"ocs_session_id": session_id, "op": "entity_schema", "parameters": {"type": "Text"}},
                    )
                    txt_props = parse_mcp_json(txt_schema_resp).get("schema", {}).get("properties", {})
                    rot_key = next((k for k in ("rotation_deg", "angle_deg", "angle", "rotation") if k in txt_props), None)

                    for level, segs in zip(cs.levels, cs.allsegs):
                        is_major = np.isclose(round(level / major_int) * major_int, level, atol=1e-6)
                        layer = maj_layer if is_major else min_layer
                        color = maj_color if is_major else min_color
                        lw = maj_lw if is_major else min_lw
                        width = maj_width if is_major else min_width

                        for seg in segs:
                            if len(seg) < 2:
                                continue
                            smoothed = chaikin_smooth(seg, iterations=smooth_passes)
                            verts_2d = [[round(float(x), 4), round(float(y), 4)] for x, y in smoothed]

                            ent = {"type": "LwPolyline", "layer": layer, "color": color, "lineweight": lw, "vertices": verts_2d}
                            if width > 0:
                                ent["constant_width"] = width
                            entities_to_create.append(ent)

                            if is_major:
                                major_count += 1
                            else:
                                minor_count += 1

                            if is_major and add_labels and len(smoothed) >= 2:
                                diffs = np.diff(smoothed, axis=0)
                                total_len = float(np.sum(np.hypot(diffs[:, 0], diffs[:, 1])))
                                if total_len >= lbl_min_len:
                                    mid_idx = len(smoothed) // 2
                                    ax_x, ax_y = get_xy(smoothed[max(0, mid_idx - 1)])
                                    bx_x, bx_y = get_xy(smoothed[min(len(smoothed) - 1, mid_idx)])
                                    mid_x = 0.5 * (ax_x + bx_x)
                                    mid_y = 0.5 * (ax_y + bx_y)
                                    angle_deg = math.degrees(math.atan2(float(bx_y - ax_y), float(bx_x - ax_x)))
                                    if angle_deg > 90.0:
                                        angle_deg -= 180.0
                                    elif angle_deg < -90.0:
                                        angle_deg += 180.0

                                    txt_ent = {
                                        "type": "Text",
                                        "layer": lbl_layer,
                                        "color": lbl_color,
                                        "position": [round(float(mid_x), 4), round(float(mid_y), 4)],
                                        "height": lbl_height,
                                        "value": f"{level:.{lbl_decimals}f}",
                                    }
                                    if rot_key:
                                        txt_ent[rot_key] = round(angle_deg, 2)
                                    entities_to_create.append(txt_ent)
                                    label_count += 1                                   


                    plt.close(fig)
                    self.log(f"Prepared {major_count} major, {minor_count} minor contours, and {label_count} labels.")
                    if not entities_to_create:
                        self.log("Nothing to draw.")
                        return

                    MAX_BATCH_BYTES = 450_000  # Safely under OCS's 1 MiB IPC limit
                    batches = []
                    current_batch = []
                    current_bytes = 200  # JSON envelope overhead

                    for ent in entities_to_create:
                        ent_bytes = len(json.dumps(ent, separators=(",", ":")).encode("utf-8")) + 2
                        if current_batch and (current_bytes + ent_bytes > MAX_BATCH_BYTES):
                            batches.append(current_batch)
                            current_batch = [ent]
                            current_bytes = 200 + ent_bytes
                        else:
                            current_batch.append(ent)
                            current_bytes += ent_bytes

                    if current_batch:
                        batches.append(current_batch)

                    for idx, chunk in enumerate(batches, start=1):
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
                        self.log(f"Sent batch {idx}/{len(batches)} ({len(chunk)} entities)...")

                    self.log("Success! Boundary-clipped contours & labels are now live in Open CAD Studio.")

        self._run_async_in_thread(_generate)


if __name__ == "__main__":
    root = tk.Tk()
    app = ContourGeneratorGUI(root)
    root.mainloop()
