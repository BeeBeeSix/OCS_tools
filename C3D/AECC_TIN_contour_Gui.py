import asyncio
import threading
import tkinter as tk
from tkinter import messagebox
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

from contour_config import LINEWEIGHT_OPTIONS, OCS_BIN
from contour_gui_widgets import build_contour_ui
from ocs_contour_client import (
    clear_contour_layers,
    fetch_selected_handle,
    fetch_sessions_and_tin,
    generate_tin_contours,
)


class ContourGeneratorGUI:
    def __init__(self, root: tk.Tk):
        self.root = root
        self.root.title("Open CAD Studio — AECC_TIN_Surface Contour Styler")
        self.root.geometry("660x815")
        self.root.resizable(False, False)

        self.sessions_map = {}
        self.session_var = tk.StringVar(value="Click 'Refresh' to connect...")
        self.surface_handle_var = tk.StringVar(value="")
        self.surface_info_var = tk.StringVar(value="Surface stats: click Refresh to inspect AECC_TIN_Surface")
        self.boundary_handle_var = tk.StringVar(value="")
        self.xml_path_var = tk.StringVar(value="")
        self.max_edge_var = tk.DoubleVar(value=120.0)

        self.minor_int_var = tk.DoubleVar(value=1.0)
        self.major_int_var = tk.DoubleVar(value=5.0)
        self.smooth_var = tk.IntVar(value=1)

        self.maj_layer_var = tk.StringVar(value="C-TOPO-MAJR")
        self.maj_color_preset_var = tk.StringVar(value="1 - Red")
        self.maj_aci_var = tk.IntVar(value=1)
        self.maj_lw_var = tk.StringVar(value="0.35 mm (35)")
        self.maj_width_var = tk.DoubleVar(value=0.4)

        self.min_layer_var = tk.StringVar(value="C-TOPO-MINR")
        self.min_color_preset_var = tk.StringVar(value="8 - Dark Gray")
        self.min_aci_var = tk.IntVar(value=8)
        self.min_lw_var = tk.StringVar(value="0.13 mm (13)")
        self.min_width_var = tk.DoubleVar(value=0.0)

        self.add_labels_var = tk.BooleanVar(value=True)
        self.lbl_layer_var = tk.StringVar(value="C-TOPO-TEXT")
        self.lbl_height_var = tk.DoubleVar(value=2.5)
        self.lbl_aci_var = tk.IntVar(value=7)
        self.lbl_min_len_var = tk.DoubleVar(value=40.0)
        self.lbl_decimals_var = tk.IntVar(value=0)
        self.replace_existing_var = tk.BooleanVar(value=True)

        build_contour_ui(self)
        self.refresh_sessions()

    def log(self, msg: str):
        def _append():
            self.log_text.configure(state="normal")
            self.log_text.insert("end", msg + "\n")
            self.log_text.see("end")
            self.log_text.configure(state="disabled")
        self.root.after(0, _append)

    def _run_bg(self, coro_fn):
        def _worker():
            self.root.after(0, lambda: (self.run_btn.configure(state="disabled"), self.clear_btn.configure(state="disabled")))
            try:
                asyncio.run(coro_fn())
            except BaseException as exc:
                while hasattr(exc, "exceptions") and exc.exceptions:
                    exc = next(iter(exc.exceptions))
                err_msg = f"{type(exc).__name__}: {exc}"
                self.log(f"ERROR: {err_msg}")
                self.root.after(0, lambda m=err_msg: messagebox.showerror("Open CAD Studio Error", m))
            finally:
                self.root.after(0, lambda: (self.run_btn.configure(state="normal"), self.clear_btn.configure(state="normal")))
        threading.Thread(target=_worker, daemon=True).start()

    def refresh_sessions(self):
        async def _fetch():
            self.log("Querying active Open CAD Studio sessions & AECC_TIN_Surface...")
            new_map, tin_h, stats = await fetch_sessions_and_tin()

            def _ui():
                self.sessions_map = new_map
                labels = list(new_map.keys())
                self.session_combo["values"] = labels
                if labels:
                    first_lbl = next(iter(labels))
                    self.session_var.set(first_lbl)
                    if tin_h:
                        self.surface_handle_var.set(tin_h)
                    self.surface_info_var.set(stats)
                    self.log(f"Connected: {first_lbl} | {stats}")
                else:
                    self.session_var.set("No active GUI session found")
            self.root.after(0, _ui)
        self._run_bg(_fetch)

    def _pick_handle(self, target_var: tk.StringVar, label: str):
        sid = self.sessions_map.get(self.session_var.get())
        if not sid:
            return

        async def _pick():
            h = await fetch_selected_handle(sid)
            if h:
                self.root.after(0, lambda: target_var.set(h))
                self.log(f"Selected {label} handle: {h}")
            else:
                self.log(f"Select a {label} in Open CAD Studio first, then click Pick.")
        self._run_bg(_pick)

    def run_clear_only(self):
        sid = self.sessions_map.get(self.session_var.get())
        if not sid:
            return

        async def _clear():
            async with stdio_client(StdioServerParameters(command=OCS_BIN, args=["--mcp"])) as (r, w):
                async with ClientSession(r, w) as s:
                    await s.initialize()
                    layers = [self.maj_layer_var.get().strip(), self.min_layer_var.get().strip(), self.lbl_layer_var.get().strip()]
                    await clear_contour_layers(s, sid, layers, self.surface_handle_var.get(), self.log)
                    self.log("Target layers cleared.")
        self._run_bg(_clear)

    def run_generation(self):
        sid = self.sessions_map.get(self.session_var.get())
        tin_handle = self.surface_handle_var.get().strip()
        if not sid or not tin_handle:
            messagebox.showerror("Missing Input", "Connect to a session and specify the AECC_TIN_Surface handle.")
            return
        cfg = {
            "tin_handle": tin_handle,
            "bnd_handle": self.boundary_handle_var.get().strip(),
            "xml_path": self.xml_path_var.get().strip(),
            "max_edge": float(self.max_edge_var.get()),
            "minor_int": float(self.minor_int_var.get()),
            "major_int": float(self.major_int_var.get()),
            "smooth_passes": int(self.smooth_var.get()),
            "maj_layer": self.maj_layer_var.get().strip() or "C-TOPO-MAJR",
            "maj_color": int(self.maj_aci_var.get()),
            "maj_lw": LINEWEIGHT_OPTIONS[self.maj_lw_var.get()],
            "maj_width": float(self.maj_width_var.get()),
            "min_layer": self.min_layer_var.get().strip() or "C-TOPO-MINR",
            "min_color": int(self.min_aci_var.get()),
            "min_lw": LINEWEIGHT_OPTIONS[self.min_lw_var.get()],
            "min_width": float(self.min_width_var.get()),
            "add_labels": bool(self.add_labels_var.get()),
            "lbl_layer": self.lbl_layer_var.get().strip() or "C-TOPO-TEXT",
            "lbl_height": float(self.lbl_height_var.get()),
            "lbl_color": int(self.lbl_aci_var.get()),
            "lbl_min_len": float(self.lbl_min_len_var.get()),
            "lbl_decimals": int(self.lbl_decimals_var.get()),
            "replace_existing": bool(self.replace_existing_var.get()),
        }
        self._run_bg(lambda: generate_tin_contours(sid, cfg, self.log))


if __name__ == "__main__":
    root = tk.Tk()
    ContourGeneratorGUI(root)
    root.mainloop()
