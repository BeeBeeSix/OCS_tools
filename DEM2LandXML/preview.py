import tkinter as tk
import matplotlib.cm as cm
import numpy as np
from PIL import Image, ImageTk
import rasterio


class DemPreviewCanvas(tk.Frame):

    def __init__(self, parent, on_poly_change=None):
        super().__init__(parent)
        self.on_poly_change = on_poly_change
        self.cw, self.ch = 540, 400
        self.bounds, self.crs, self.base_img = None, None, None
        self.zoom, self.pan_x, self.pan_y = 1.0, 0.0, 0.0
        self.poly_geo = []

        self.canvas = tk.Canvas(
            self, width=self.cw, height=self.ch, bg="#1e1e1e", cursor="crosshair"
        )
        self.canvas.pack(fill="both", expand=True)
        self.canvas.bind("<Button-1>", self.add_vertex)
        self.canvas.bind("<Button-3>", lambda e: self.clear_polygon())
        self.canvas.bind("<MouseWheel>", self.on_zoom)
        self.canvas.bind("<ButtonPress-2>", self.start_pan)
        self.canvas.bind("<B2-Motion>", self.do_pan)
        self.canvas.bind("<Shift-ButtonPress-1>", self.start_pan)
        self.canvas.bind("<Shift-B1-Motion>", self.do_pan)

        bar = tk.Frame(self)
        bar.pack(fill="x", pady=(4, 0))
        tk.Label(
            bar,
            text="Click: Add pt | Wheel: Zoom | Mid-Drag: Pan | R-Click: Clear",
            fg="#555",
        ).pack(side="left")
        tk.Button(
            bar, text="Reset View", command=self.reset_view, width=10
        ).pack(side="right", padx=(4, 0))
        tk.Button(
            bar, text="Clear Poly", command=self.clear_polygon, width=10
        ).pack(side="right")

    def load_dem(self, tif_path):
        self.poly_geo.clear()
        with rasterio.open(tif_path) as src:
            self.bounds, self.crs = src.bounds, src.crs
            step = max(1, max(src.height // 800, src.width // 800))
            arr = src.read(1)[::step, ::step].astype(float)
            if src.nodata is not None:
                arr[arr == src.nodata] = np.nan

        valid = ~np.isnan(arr)
        if np.any(valid):
            vmin, vmax = np.nanpercentile(arr, 2), np.nanpercentile(arr, 98)
            norm = np.clip((arr - vmin) / max(vmax - vmin, 1e-6), 0, 1)
            rgba = (cm.terrain(norm) * 255).astype(np.uint8)
            rgba[~valid] = [30, 30, 30, 255]
        else:
            rgba = np.full(
                (arr.shape[0], arr.shape[1], 4), [30, 30, 30, 255], dtype=np.uint8
            )

        self.base_img = Image.fromarray(rgba)
        self.reset_view()

    def reset_view(self):
        self.zoom, self.pan_x, self.pan_y = 1.0, 0.0, 0.0
        self.render()

    def set_polygon_geo(self, coords):
        self.poly_geo = list(coords)
        self.render()

    def clear_polygon(self):
        self.poly_geo.clear()
        self.render()

    def on_zoom(self, event):
        if not self.base_img:
            return
        factor = 1.25 if event.delta > 0 else 0.8
        new_z = max(1.0, min(20.0, self.zoom * factor))
        ratio = new_z / self.zoom
        self.pan_x = event.x - (event.x - self.pan_x) * ratio
        self.pan_y = event.y - (event.y - self.pan_y) * ratio
        self.zoom = new_z
        self.render()

    def start_pan(self, event):
        self._drag_x, self._drag_y = event.x, event.y

    def do_pan(self, event):
        self.pan_x += event.x - self._drag_x
        self.pan_y += event.y - self._drag_y
        self._drag_x, self._drag_y = event.x, event.y
        self.render()

    def add_vertex(self, event):
        if not self.bounds:
            return
        w, h = self.cw * self.zoom, self.ch * self.zoom
        u, v = (event.x - self.pan_x) / w, (event.y - self.pan_y) / h
        gx = self.bounds.left + u * (self.bounds.right - self.bounds.left)
        gy = self.bounds.top - v * (self.bounds.top - self.bounds.bottom)
        self.poly_geo.append((gx, gy))
        self.render()

    def geo_to_canvas(self, gx, gy):
        u = (gx - self.bounds.left) / (self.bounds.right - self.bounds.left)
        v = (self.bounds.top - gy) / (self.bounds.top - self.bounds.bottom)
        return (
            self.pan_x + u * self.cw * self.zoom,
            self.pan_y + v * self.ch * self.zoom,
        )

    def render(self):
        self.canvas.delete("all")
        if self.base_img:
            nw, nh = int(self.cw * self.zoom), int(self.ch * self.zoom)
            resized = self.base_img.resize((nw, nh), Image.Resampling.BILINEAR)
            self.tk_img = ImageTk.PhotoImage(resized)
            self.canvas.create_image(
                self.pan_x, self.pan_y, anchor="nw", image=self.tk_img
            )

        pts = [self.geo_to_canvas(gx, gy) for gx, gy in self.poly_geo]
        if len(pts) >= 3:
            self.canvas.create_polygon(
                [c for p in pts for c in p],
                outline="#ff3333",
                fill="#ff3333",
                stipple="gray25",
                width=2,
            )
        elif len(pts) == 2:
            self.canvas.create_line(pts[0], pts[1], fill="#ff3333", width=2)
        for x, y in pts:
            self.canvas.create_oval(
                x - 3, y - 3, x + 3, y + 3, fill="yellow", outline="black"
            )
        if self.on_poly_change:
            self.on_poly_change(len(pts))
