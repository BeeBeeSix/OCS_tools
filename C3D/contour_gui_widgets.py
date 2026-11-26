import tkinter as tk
from tkinter import filedialog, ttk
from contour_config import ACI_PRESETS, LINEWEIGHT_OPTIONS


def build_contour_ui(app):
    pad = {"padx": 10, "pady": 4}

    # 1. Live Session & AECC_TIN_Surface
    conn_frame = ttk.LabelFrame(app.root, text="1. Live Session & AECC_TIN_Surface", padding=8)
    conn_frame.pack(fill="x", **pad)

    ttk.Label(conn_frame, text="Active Session:").grid(row=0, column=0, sticky="w")
    app.session_combo = ttk.Combobox(conn_frame, textvariable=app.session_var, state="readonly", width=52)
    app.session_combo.grid(row=0, column=1, columnspan=3, padx=6, sticky="w")
    ttk.Button(conn_frame, text="Refresh", command=app.refresh_sessions).grid(row=0, column=4)

    ttk.Label(conn_frame, text="TIN Handle:").grid(row=1, column=0, sticky="w", pady=(6, 0))
    tin_row = ttk.Frame(conn_frame)
    tin_row.grid(row=1, column=1, columnspan=2, sticky="w", pady=(6, 0))
    ttk.Entry(tin_row, textvariable=app.surface_handle_var, width=9).pack(side="left", padx=(6, 4))
    ttk.Button(
        tin_row, text="Pick Selected TIN", command=lambda: app._pick_handle(app.surface_handle_var, "TIN surface")
    ).pack(side="left")

    ttk.Label(conn_frame, text="Boundary (opt):").grid(row=1, column=3, sticky="e", pady=(6, 0))
    b_row = ttk.Frame(conn_frame)
    b_row.grid(row=1, column=4, sticky="w", pady=(6, 0))
    ttk.Entry(b_row, textvariable=app.boundary_handle_var, width=8).pack(side="left", padx=(4, 4))
    ttk.Button(
        b_row, text="Pick", command=lambda: app._pick_handle(app.boundary_handle_var, "boundary polyline")
    ).pack(side="left")

    # Row 2: LandXML Source Picker
    ttk.Label(conn_frame, text="LandXML Source:").grid(row=2, column=0, sticky="w", pady=(6, 0))
    ttk.Entry(conn_frame, textvariable=app.xml_path_var, width=52).grid(
        row=2, column=1, columnspan=3, padx=6, pady=(6, 0), sticky="w"
    )
    ttk.Button(
        conn_frame,
        text="Browse...",
        command=lambda: app.xml_path_var.set(
            filedialog.askopenfilename(title="Select LandXML File", filetypes=[("LandXML", "*.xml"), ("All", "*.*")])
            or app.xml_path_var.get()
        ),
    ).grid(row=2, column=4, pady=(6, 0))

    ttk.Label(conn_frame, textvariable=app.surface_info_var, foreground="#1E5AA8").grid(
        row=3, column=0, columnspan=5, sticky="w", pady=(6, 0)
    )

    # 2. Intervals & Clipping
    int_frame = ttk.LabelFrame(app.root, text="2. Contour Intervals & TIN Clipping", padding=8)
    int_frame.pack(fill="x", **pad)
    ttk.Label(int_frame, text="Minor (m):").grid(row=0, column=0, sticky="w")
    ttk.Entry(int_frame, textvariable=app.minor_int_var, width=8).grid(row=0, column=1, sticky="w", padx=(4, 12))
    ttk.Label(int_frame, text="Major (m):").grid(row=0, column=2, sticky="w")
    ttk.Entry(int_frame, textvariable=app.major_int_var, width=8).grid(row=0, column=3, sticky="w", padx=(4, 12))
    ttk.Label(int_frame, text="Smooth:").grid(row=0, column=4, sticky="w")
    ttk.Spinbox(int_frame, from_=0, to=3, textvariable=app.smooth_var, width=4).grid(row=0, column=5, sticky="w", padx=(4, 12))
    ttk.Label(int_frame, text="Max Edge (m):").grid(row=0, column=6, sticky="w")
    ttk.Entry(int_frame, textvariable=app.max_edge_var, width=8).grid(row=0, column=7, sticky="w", padx=4)

    # 3 & 4. Styling Sections
    _build_style_section(
        app.root, "3. Major Contours Styling", app.maj_layer_var, app.maj_color_preset_var,
        app.maj_aci_var, "#FF0000", app.maj_lw_var, app.maj_width_var, pad
    )
    _build_style_section(
        app.root, "4. Minor Contours Styling", app.min_layer_var, app.min_color_preset_var,
        app.min_aci_var, "#808080", app.min_lw_var, app.min_width_var, pad
    )

    # 5. Labels
    lbl_frame = ttk.LabelFrame(app.root, text="5. Major Contour Elevation Labels", padding=8)
    lbl_frame.pack(fill="x", **pad)
    ttk.Checkbutton(lbl_frame, text="Add Elevation Labels on Major Contours", variable=app.add_labels_var).grid(
        row=0, column=0, columnspan=3, sticky="w"
    )
    ttk.Label(lbl_frame, text="Label Layer:").grid(row=1, column=0, sticky="w", pady=(6, 0))
    ttk.Entry(lbl_frame, textvariable=app.lbl_layer_var, width=22).grid(row=1, column=1, sticky="w", padx=6, pady=(6, 0))
    ttk.Label(lbl_frame, text="Text Height:").grid(row=1, column=2, sticky="w", pady=(6, 0))
    ttk.Entry(lbl_frame, textvariable=app.lbl_height_var, width=8).grid(row=1, column=3, sticky="w", padx=4, pady=(6, 0))
    ttk.Label(lbl_frame, text="ACI Color:").grid(row=1, column=4, sticky="w", pady=(6, 0))
    ttk.Entry(lbl_frame, textvariable=app.lbl_aci_var, width=6).grid(row=1, column=5, sticky="w", padx=4, pady=(6, 0))
    ttk.Label(lbl_frame, text="Min Length (m):").grid(row=2, column=0, sticky="w", pady=(6, 0))
    ttk.Entry(lbl_frame, textvariable=app.lbl_min_len_var, width=10).grid(row=2, column=1, sticky="w", padx=6, pady=(6, 0))
    ttk.Label(lbl_frame, text="Decimals:").grid(row=2, column=2, sticky="w", pady=(6, 0))
    ttk.Spinbox(lbl_frame, from_=0, to=3, textvariable=app.lbl_decimals_var, width=5).grid(row=2, column=3, sticky="w", padx=4, pady=(6, 0))

    # 6. Buttons & Log
    act_frame = ttk.Frame(app.root, padding=(10, 4))
    act_frame.pack(fill="x")
    ttk.Checkbutton(
        act_frame, text="Clear existing entities on target contour/label layers first", variable=app.replace_existing_var
    ).pack(anchor="w", pady=(0, 6))
    btn_row = ttk.Frame(act_frame)
    btn_row.pack(fill="x")
    app.run_btn = ttk.Button(btn_row, text="Generate & Style TIN Contours", command=app.run_generation)
    app.run_btn.pack(side="left", padx=(0, 8), ipadx=10, ipady=4)
    app.clear_btn = ttk.Button(btn_row, text="Clear Contour Layers Only", command=app.run_clear_only)
    app.clear_btn.pack(side="left", ipadx=6, ipady=4)

    log_frame = ttk.LabelFrame(app.root, text="Status Log", padding=8)
    log_frame.pack(fill="both", expand=True, **pad)
    app.log_text = tk.Text(log_frame, height=8, wrap="word", state="disabled", font=("Consolas", 9))
    app.log_text.pack(fill="both", expand=True)


def _build_style_section(parent, title, lyr_v, pre_v, aci_v, hex_bg, lw_v, w_v, pad):
    f = ttk.LabelFrame(parent, text=title, padding=8)
    f.pack(fill="x", **pad)
    ttk.Label(f, text="Layer Name:").grid(row=0, column=0, sticky="w")
    ttk.Entry(f, textvariable=lyr_v, width=22).grid(row=0, column=1, sticky="w", padx=6)
    ttk.Label(f, text="Color Preset:").grid(row=0, column=2, sticky="w")
    cb = ttk.Combobox(f, textvariable=pre_v, values=list(ACI_PRESETS.keys()), state="readonly", width=16)
    cb.grid(row=0, column=3, sticky="w", padx=4)
    swatch = tk.Label(f, text="   ", bg=hex_bg, relief="solid", borderwidth=1)
    swatch.grid(row=0, column=6, padx=4)

    def _on_select(_evt):
        aci_code, hex_color = ACI_PRESETS[pre_v.get()]
        aci_v.set(aci_code)
        swatch.configure(bg=hex_color)

    cb.bind("<<ComboboxSelected>>", _on_select)
    ttk.Label(f, text="ACI #:").grid(row=0, column=4, sticky="w", padx=(6, 0))
    ttk.Entry(f, textvariable=aci_v, width=5).grid(row=0, column=5, sticky="w", padx=4)
    ttk.Label(f, text="Lineweight:").grid(row=1, column=0, sticky="w", pady=(6, 0))
    ttk.Combobox(f, textvariable=lw_v, values=list(LINEWEIGHT_OPTIONS.keys()), state="readonly", width=19).grid(
        row=1, column=1, sticky="w", padx=6, pady=(6, 0)
    )
    ttk.Label(f, text="Constant Width:").grid(row=1, column=2, sticky="w", pady=(6, 0))
    ttk.Entry(f, textvariable=w_v, width=10).grid(row=1, column=3, sticky="w", padx=4, pady=(6, 0))
