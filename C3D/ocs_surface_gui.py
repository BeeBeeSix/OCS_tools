import asyncio
import os
import threading
import tkinter as tk
from tkinter import filedialog, messagebox, scrolledtext, ttk

from landxml_parser import export_to_standard_landxml, parse_landxml_surfaces
from ocs_mcp_client import build_surface_in_ocs

DEFAULT_OCS_BIN = r"C:\Program Files\Open CAD Studio\OpenCADStudio.exe"


class LandXMLToOCSApp:
    def __init__(self, root: tk.Tk):
        self.root = root
        self.root.title("LandXML / 3DFace → Open CAD Studio (AECC_TIN_SURFACE)")
        self.root.geometry("700x520")
        self.parsed_surfaces = []
        self._build_ui()

    def _build_ui(self):
        pad = {"padx": 10, "pady": 6}

        frm_paths = ttk.LabelFrame(self.root, text="Paths & Files", padding=10)
        frm_paths.pack(fill="x", **pad)
        frm_paths.columnconfigure(1, weight=1)

        ttk.Label(frm_paths, text="OpenCADStudio.exe:").grid(row=0, column=0, sticky="w")
        self.var_ocs_bin = tk.StringVar(value=DEFAULT_OCS_BIN)
        ttk.Entry(frm_paths, textvariable=self.var_ocs_bin).grid(row=0, column=1, sticky="ew", padx=5)
        ttk.Button(frm_paths, text="Browse...", command=self._browse_ocs).grid(row=0, column=2)

        ttk.Label(frm_paths, text="Input LandXML:").grid(row=1, column=0, sticky="w", pady=(6, 0))
        self.var_xml_path = tk.StringVar()
        ttk.Entry(frm_paths, textvariable=self.var_xml_path).grid(row=1, column=1, sticky="ew", padx=5, pady=(6, 0))
        ttk.Button(frm_paths, text="Browse...", command=self._browse_xml).grid(row=1, column=2, pady=(6, 0))

        ttk.Label(frm_paths, text="Save DWG As (opt):").grid(row=2, column=0, sticky="w", pady=(6, 0))
        self.var_dwg_path = tk.StringVar()
        ttk.Entry(frm_paths, textvariable=self.var_dwg_path).grid(row=2, column=1, sticky="ew", padx=5, pady=(6, 0))
        ttk.Button(frm_paths, text="Browse...", command=self._browse_dwg).grid(row=2, column=2, pady=(6, 0))

        frm_surf = ttk.LabelFrame(self.root, text="LandXML Surface", padding=10)
        frm_surf.pack(fill="x", **pad)
        frm_surf.columnconfigure(1, weight=1)

        ttk.Label(frm_surf, text="Select Surface:").grid(row=0, column=0, sticky="w")
        self.var_surface_choice = tk.StringVar()
        self.cmb_surfaces = ttk.Combobox(frm_surf, textvariable=self.var_surface_choice, state="readonly")
        self.cmb_surfaces.grid(row=0, column=1, sticky="ew", padx=5)

        self.var_swap_xy = tk.BooleanVar(value=True)
        ttk.Checkbutton(
            frm_surf, text="Swap Northing/Easting (LandXML Y,X,Z → CAD X,Y,Z)", variable=self.var_swap_xy
        ).grid(row=1, column=0, columnspan=2, sticky="w", pady=(6, 0))

        frm_act = ttk.Frame(self.root)
        frm_act.pack(fill="x", **pad)

        ttk.Button(frm_act, text="Convert to LandXML Surface...", command=self._export_clean_landxml).pack(side="left")
        self.btn_run = ttk.Button(frm_act, text="Build AECC_TIN_SURFACE in OCS", command=self._start_import)
        self.btn_run.pack(side="left", padx=(8, 0))

        self.progress = ttk.Progressbar(frm_act, mode="determinate")
        self.progress.pack(side="right", fill="x", expand=True, padx=(10, 0))

        frm_log = ttk.LabelFrame(self.root, text="Session Log", padding=8)
        frm_log.pack(fill="both", expand=True, **pad)
        self.txt_log = scrolledtext.ScrolledText(frm_log, height=12, state="disabled", font=("Consolas", 9))
        self.txt_log.pack(fill="both", expand=True)

    def log(self, msg: str):
        def _append():
            self.txt_log.configure(state="normal")
            self.txt_log.insert("end", msg + "\n")
            self.txt_log.see("end")
            self.txt_log.configure(state="disabled")
        self.root.after(0, _append)

    def set_progress(self, val: float):
        self.root.after(0, lambda: self.progress.configure(value=val))

    def _browse_ocs(self):
        p = filedialog.askopenfilename(title="Select OpenCADStudio.exe", filetypes=[("Executable", "*.exe")])
        if p:
            self.var_ocs_bin.set(p)

    def _browse_xml(self):
        p = filedialog.askopenfilename(title="Select LandXML File", filetypes=[("LandXML", "*.xml"), ("All", "*.*")])
        if not p:
            return
        self.var_xml_path.set(p)
        try:
            self.parsed_surfaces = parse_landxml_surfaces(p)
            if not self.parsed_surfaces:
                messagebox.showwarning("No Surfaces", "No <Surface> elements found in this file.")
                return
            labels = [f"{s['name']} ({len(s['points'])} pts, {len(s['faces'])} faces)" for s in self.parsed_surfaces]
            self.cmb_surfaces["values"] = labels
            self.cmb_surfaces.current(0)
            self.log(f"Loaded '{os.path.basename(p)}' — {labels}.")
        except Exception as e:
            messagebox.showerror("Parse Error", str(e))

    def _browse_dwg(self):
        p = filedialog.asksaveasfilename(title="Save DWG As", defaultextension=".dwg", filetypes=[("DWG", "*.dwg")])
        if p:
            self.var_dwg_path.set(p)

    def _get_selected_surface(self):
        if not self.parsed_surfaces:
            messagebox.showwarning("No Surface", "Select a valid LandXML file first.")
            return None
        return self.parsed_surfaces[max(self.cmb_surfaces.current(), 0)]

    def _export_clean_landxml(self):
        surf = self._get_selected_surface()
        if not surf:
            return
        out = filedialog.asksaveasfilename(
            title="Save Standard LandXML", initialfile=f"{surf['name']}_surface.xml", defaultextension=".xml"
        )
        if out:
            export_to_standard_landxml(surf, out, swap_xy=self.var_swap_xy.get())
            self.log(f"Saved standard LandXML surface to '{out}'.")

    def _start_import(self):
        ocs_bin = self.var_ocs_bin.get().strip()
        surf = self._get_selected_surface()
        if not surf or not os.path.isfile(ocs_bin):
            messagebox.showerror("Missing Input", "Verify OpenCADStudio.exe path and LandXML selection.")
            return
        self.btn_run.configure(state="disabled")
        self.set_progress(0)
        threading.Thread(
            target=self._worker,
            args=(ocs_bin, surf, self.var_dwg_path.get().strip(), self.var_swap_xy.get()),
            daemon=True,
        ).start()

    def _worker(self, ocs_bin: str, surf: dict, dwg_out: str, swap_xy: bool):
        try:
            asyncio.run(build_surface_in_ocs(ocs_bin, surf, dwg_out, swap_xy, self.log, self.set_progress))
        except Exception as e:
            self.log(f"ERROR: {e}")
            self.root.after(0, lambda: messagebox.showerror("MCP Error", str(e)))
        finally:
            self.root.after(0, lambda: self.btn_run.configure(state="normal"))


if __name__ == "__main__":
    root = tk.Tk()
    LandXMLToOCSApp(root)
    root.mainloop()
