import os
from pathlib import Path
import threading
import tkinter as tk
from tkinter import filedialog, messagebox, ttk
from boundary_loader import load_boundary_polygon
from converter import convert_dem_to_landxml
from preview import DemPreviewCanvas


class DemToLandXMLApp:

    def __init__(self, root):
        self.root = root
        self.root.title("GeoTIFF DEM to LandXML Converter")
        self.root.resizable(False, False)

        self.input_path = tk.StringVar()
        self.output_path = tk.StringVar()
        self.surface_name = tk.StringVar(value="DEM_Surface")
        self.step_var = tk.IntVar(value=1)
        self.poly_info = tk.StringVar(value="Crop: Full DEM (0 points)")
        self.status_var = tk.StringVar(value="Ready.")
        self.build_ui()

    def build_ui(self):
        main = tk.Frame(self.root, padx=12, pady=10)
        main.pack()
        left = tk.Frame(main, width=340)
        left.pack(side="left", fill="y", padx=(0, 12))

        tk.Label(left, text="Input GeoTIFF DEM:", anchor="w").pack(fill="x")
        in_f = tk.Frame(left)
        in_f.pack(fill="x", pady=(2, 8))
        tk.Entry(in_f, textvariable=self.input_path, width=30).pack(
            side="left", fill="x", expand=True
        )
        tk.Button(in_f, text="Browse", command=self.browse_in, width=8).pack(
            side="right", padx=(4, 0)
        )

        tk.Label(left, text="Output LandXML File:", anchor="w").pack(fill="x")
        out_f = tk.Frame(left)
        out_f.pack(fill="x", pady=(2, 8))
        tk.Entry(out_f, textvariable=self.output_path, width=30).pack(
            side="left", fill="x", expand=True
        )
        tk.Button(out_f, text="Save As", command=self.browse_out, width=8).pack(
            side="right", padx=(4, 0)
        )

        opts = tk.LabelFrame(left, text="Settings & Boundary", padx=8, pady=8)
        opts.pack(fill="x", pady=6)
        tk.Label(opts, text="Surface Name:").grid(row=0, column=0, sticky="w")
        tk.Entry(opts, textvariable=self.surface_name, width=20).grid(
            row=0, column=1, padx=6, pady=3
        )
        tk.Label(opts, text="Subsample Step:").grid(row=1, column=0, sticky="w")
        tk.Spinbox(
            opts, from_=1, to=100, textvariable=self.step_var, width=8
        ).grid(row=1, column=1, sticky="w", padx=6, pady=3)
        tk.Button(
            opts,
            text="Import Boundary (.shp / .geojson)",
            command=self.import_boundary,
        ).grid(row=2, column=0, columnspan=2, sticky="ew", pady=(8, 4))
        tk.Label(opts, textvariable=self.poly_info, fg="#005a9e").grid(
            row=3, column=0, columnspan=2, sticky="w"
        )

        self.btn = tk.Button(
            left,
            text="Convert to LandXML",
            command=self.start,
            bg="#0078D7",
            fg="white",
            font=("Segoe UI", 10, "bold"),
            height=2,
        )
        self.btn.pack(fill="x", pady=(14, 6))

        self.progress = ttk.Progressbar(left, mode="indeterminate")
        self.progress.pack(fill="x", pady=4)
        tk.Label(
            left, textvariable=self.status_var, anchor="w", wraplength=320
        ).pack(fill="x")

        right = tk.LabelFrame(
            main, text="DEM Preview (Zoom, Pan & Crop)", padx=6, pady=6
        )
        right.pack(side="right")
        self.preview = DemPreviewCanvas(right, on_poly_change=self.update_poly)
        self.preview.pack()

    def update_poly(self, count):
        msg = (
            f"Crop: Full DEM ({count} pts)"
            if count < 3
            else f"Crop: Active Polygon ({count} vertices)"
        )
        self.poly_info.set(msg)

    def browse_in(self):
        path = filedialog.askopenfilename(
            filetypes=[("GeoTIFF", "*.tif *.tiff"), ("All", "*.*")]
        )
        if path:
            self.input_path.set(path)
            self.output_path.set(str(Path(path).with_suffix(".xml")))
            self.preview.load_dem(path)

    def browse_out(self):
        path = filedialog.asksaveasfilename(
            defaultextension=".xml", filetypes=[("LandXML", "*.xml")]
        )
        if path:
            self.output_path.set(path)

    def import_boundary(self):
        if not self.preview.bounds:
            messagebox.showwarning("Notice", "Please load a GeoTIFF DEM first.")
            return
        path = filedialog.askopenfilename(
            filetypes=[
                ("Boundary", "*.shp *.geojson *.json"),
                ("All", "*.*"),
            ]
        )
        if path:
            try:
                coords = load_boundary_polygon(path, self.preview.crs)
                self.preview.set_polygon_geo(coords)
            except Exception as e:
                messagebox.showerror("Boundary Error", str(e))

    def start(self):
        in_f, out_f = (
            self.input_path.get().strip(),
            self.output_path.get().strip(),
        )
        if not in_f or not os.path.exists(in_f) or not out_f:
            messagebox.showerror(
                "Error", "Please select valid input and output files."
            )
            return
        self.btn.config(state="disabled")
        self.progress.start(10)
        poly = (
            list(self.preview.poly_geo)
            if len(self.preview.poly_geo) >= 3
            else None
        )
        args = (
            in_f,
            out_f,
            self.surface_name.get().strip() or "DEM_Surface",
            self.step_var.get(),
            poly,
        )
        threading.Thread(target=self.worker, args=args, daemon=True).start()

    def worker(self, in_f, out_f, name, step, poly):
        try:
            cb = lambda msg: self.root.after(
                0, lambda: self.status_var.set(msg)
            )
            convert_dem_to_landxml(in_f, out_f, name, step, poly, cb)
            self.root.after(0, self.done, out_f, None)
        except Exception as e:
            self.root.after(0, self.done, None, str(e))

    def done(self, out_f, err):
        self.progress.stop()
        self.btn.config(state="normal")
        self.status_var.set("Error occurred." if err else "Complete!")
        if err:
            messagebox.showerror("Failed", err)
        else:
            messagebox.showinfo("Success", f"Saved to:\n{out_f}")


if __name__ == "__main__":
    root = tk.Tk()
    DemToLandXMLApp(root)
    root.mainloop()
