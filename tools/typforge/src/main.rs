mod app;
mod error;
mod typ;

slint::include_modules!();

use std::rc::Rc;
use std::cell::RefCell;
use slint::{ModelRc, VecModel, StandardListViewItem, SharedPixelBuffer, Rgb8Pixel};
use typ::{TypDocument, TypPolygon, TypLine, TypPoint, Xpm};
use app::App;

// ─── Render helpers ──────────────────────────────────────────────

fn first_opaque(xpm: Option<&Xpm>) -> Option<(u8, u8, u8)> {
    let xpm = xpm?;
    for (_, c) in &xpm.palette {
        if !c.is_transparent() {
            return Some((c.r, c.g, c.b));
        }
    }
    None
}

fn solid_thumb(r: u8, g: u8, b: u8, size: u32) -> slint::Image {
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    for p in buf.make_mut_slice().iter_mut() {
        *p = Rgb8Pixel { r, g, b };
    }
    slint::Image::from_rgb8(buf)
}

fn render_polygon_thumb(poly: &TypPolygon, size: u32) -> slint::Image {
    let fill = first_opaque(poly.day_xpm.as_ref()).unwrap_or((0x88, 0x88, 0x88));
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for y in 0..size {
        for x in 0..size {
            let on_border = x == 0 || y == 0 || x == size - 1 || y == size - 1;
            pixels[(y * size + x) as usize] = if on_border {
                Rgb8Pixel { r: 0, g: 0, b: 0 }
            } else {
                Rgb8Pixel { r: fill.0, g: fill.1, b: fill.2 }
            };
        }
    }
    slint::Image::from_rgb8(buf)
}

fn render_line_thumb(line: &TypLine, size: u32) -> slint::Image {
    let lc = first_opaque(line.day_xpm.as_ref()).unwrap_or((0, 0, 0));
    let lw = (line.line_width as u32).clamp(1, size / 4);
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() {
        *p = Rgb8Pixel { r: 0xe0, g: 0xe0, b: 0xe0 };
    }
    let y_start = (size / 2).saturating_sub(lw / 2);
    for dy in 0..lw {
        let y = y_start + dy;
        if y < size {
            for x in 4..size - 4 {
                pixels[(y * size + x) as usize] = Rgb8Pixel { r: lc.0, g: lc.1, b: lc.2 };
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

fn render_point_thumb(point: &TypPoint, size: u32) -> slint::Image {
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() {
        *p = Rgb8Pixel { r: 0xff, g: 0xff, b: 0xff };
    }
    if let Some(xpm) = &point.day_xpm {
        let xw = xpm.width as u32;
        let xh = xpm.height as u32;
        let ox = size.saturating_sub(xw) / 2;
        let oy = size.saturating_sub(xh) / 2;
        for (row_i, row) in xpm.pixels.iter().enumerate() {
            let y = oy + row_i as u32;
            if y >= size { break; }
            for (col_i, &idx) in row.iter().enumerate() {
                let x = ox + col_i as u32;
                if x >= size { break; }
                if let Some((_, c)) = xpm.palette.get(idx) {
                    if !c.is_transparent() {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                    }
                }
            }
        }
    } else {
        let mid = (size / 2) as i32;
        let r = (size / 4) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let y = (mid + dy) as u32;
                    let x = (mid + dx) as u32;
                    if y < size && x < size {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: 0x44, g: 0x88, b: 0xcc };
                    }
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

// ─── UI bridge ───────────────────────────────────────────────────

fn make_item(text: impl Into<slint::SharedString>) -> StandardListViewItem {
    let mut item = StandardListViewItem::default();
    item.text = text.into();
    item
}

fn build_list_model(pairs: impl Iterator<Item = (u16, u8)>) -> ModelRc<StandardListViewItem> {
    ModelRc::new(VecModel::from(
        pairs.map(|(tc, st)| make_item(format!("0x{:02X} / 0x{:02X}", tc, st)))
             .collect::<Vec<_>>()
    ))
}

fn rebuild_gallery(doc: &TypDocument, window: &AppWindow, nav_tab: i32) {
    let items: Vec<GalleryItem> = match nav_tab {
        0 => doc.polygons.iter().enumerate().map(|(i, p)| GalleryItem {
            thumb: render_polygon_thumb(p, 56),
            name: format!("0x{:02X}/0x{:02X}", p.type_code, p.sub_type).into(),
            kind: 0,
            index: i as i32,
        }).collect(),
        1 => doc.lines.iter().enumerate().map(|(i, l)| GalleryItem {
            thumb: render_line_thumb(l, 56),
            name: format!("0x{:02X}/0x{:02X}", l.type_code, l.sub_type).into(),
            kind: 1,
            index: i as i32,
        }).collect(),
        2 => doc.points.iter().enumerate().map(|(i, p)| GalleryItem {
            thumb: render_point_thumb(p, 56),
            name: format!("0x{:02X}/0x{:02X}", p.type_code, p.sub_type).into(),
            kind: 2,
            index: i as i32,
        }).collect(),
        _ => doc.icons.iter().enumerate().map(|(i, ic)| GalleryItem {
            thumb: slint::Image::default(),
            name: format!("0x{:02X}/0x{:02X}", ic.type_code, ic.sub_type).into(),
            kind: 3,
            index: i as i32,
        }).collect(),
    };
    window.set_gallery_items(ModelRc::new(VecModel::from(items)));
}

fn update_ui_from_doc(doc: &TypDocument, window: &AppWindow) {
    window.set_meta_family_id(doc.param.family_id.to_string().into());
    window.set_meta_product_code(doc.param.product_id.to_string().into());
    window.set_meta_codepage(doc.param.codepage as i32);
    window.set_meta_header_str(doc.param.header_str.as_str().into());

    window.set_poly_count(doc.polygons.len() as i32);
    window.set_line_count(doc.lines.len() as i32);
    window.set_point_count(doc.points.len() as i32);
    window.set_poi_count(doc.icons.len() as i32);

    window.set_polygons(build_list_model(doc.polygons.iter().map(|p| (p.type_code, p.sub_type))));
    window.set_lines(build_list_model(doc.lines.iter().map(|l| (l.type_code, l.sub_type))));
    window.set_points(build_list_model(doc.points.iter().map(|p| (p.type_code, p.sub_type))));
    window.set_extra_pois(build_list_model(doc.icons.iter().map(|ic| (ic.type_code, ic.sub_type))));
    window.set_draworder(ModelRc::new(VecModel::from(
        doc.draw_order.iter()
            .map(|e| make_item(format!("L{} 0x{:02X}/0x{:02X}", e.level, e.type_code, e.sub_type)))
            .collect::<Vec<_>>()
    )));

    window.set_active_nav_tab(0);
    rebuild_gallery(doc, window, 0);
}

fn render_preview(doc: &TypDocument, kind: i32, idx: usize) -> (slint::Image, slint::Image) {
    const SZ: u32 = 128;
    let blank = solid_thumb(0x80, 0x80, 0x80, SZ);
    match kind {
        0 => match doc.polygons.get(idx) {
            Some(p) => {
                let day = render_polygon_thumb(p, SZ);
                let night = if let Some((r, g, b)) = first_opaque(p.night_xpm.as_ref()) {
                    solid_thumb(r, g, b, SZ)
                } else {
                    render_polygon_thumb(p, SZ)
                };
                (day, night)
            }
            None => (blank.clone(), blank),
        },
        1 => match doc.lines.get(idx) {
            Some(l) => { let t = render_line_thumb(l, SZ); (t.clone(), t) }
            None => (blank.clone(), blank),
        },
        2 => match doc.points.get(idx) {
            Some(p) => {
                let day = render_point_thumb(p, SZ);
                let night = if p.night_xpm.is_some() {
                    let tmp = TypPoint { day_xpm: p.night_xpm.clone(), ..p.clone() };
                    render_point_thumb(&tmp, SZ)
                } else {
                    day.clone()
                };
                (day, night)
            }
            None => (blank.clone(), blank),
        },
        _ => (blank.clone(), blank),
    }
}

// ─── main ────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let window = AppWindow::new()?;
    let app = Rc::new(RefCell::new(App::new()));

    if let Some(path_str) = args.get(1) {
        let path = std::path::Path::new(path_str);
        let mut a = app.borrow_mut();
        let res = if path.extension().map_or(false, |e| e == "typ") {
            a.import_typ(path)
        } else {
            a.open_txt(path)
        };
        match res {
            Ok(()) => {
                if let Some(doc) = &a.doc {
                    update_ui_from_doc(doc, &window);
                }
            }
            Err(e) => eprintln!("typforge: impossible d'ouvrir {:?}: {}", path, e),
        }
    }

    // on_open_file
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_open_file(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("Fichiers TYP", &["txt", "typ"])
                .set_title("Ouvrir un fichier TYP")
                .pick_file();
            if let Some(p) = picked {
                let mut a = app_c.borrow_mut();
                let res = if p.extension().map_or(false, |e| e == "typ") {
                    a.import_typ(&p)
                } else {
                    a.open_txt(&p)
                };
                match res {
                    Ok(()) => {
                        if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                            update_ui_from_doc(doc, &w);
                        }
                    }
                    Err(e) => eprintln!("typforge: erreur ouverture: {}", e),
                }
            }
        });
    }

    // on_save_file
    {
        let app_c = Rc::clone(&app);
        window.on_save_file(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("TYP texte", &["txt"])
                .set_title("Enregistrer le fichier TYP")
                .save_file();
            if let Some(p) = picked {
                if let Err(e) = app_c.borrow().save_txt(&p) {
                    eprintln!("typforge: erreur sauvegarde: {}", e);
                }
            }
        });
    }

    // on_export_typ
    {
        let app_c = Rc::clone(&app);
        window.on_export_typ(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("TYP binaire", &["typ"])
                .set_title("Exporter en .typ binaire")
                .save_file();
            if let Some(p) = picked {
                if let Err(e) = app_c.borrow().export_typ(&p) {
                    eprintln!("typforge: erreur export TYP: {}", e);
                }
            }
        });
    }

    // on_quit
    {
        let ww = window.as_weak();
        window.on_quit(move || {
            if let Some(w) = ww.upgrade() {
                w.hide().ok();
            }
        });
    }

    // on_nav_tab_changed
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_nav_tab_changed(move |tab| {
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                rebuild_gallery(doc, &w, tab);
            }
        });
    }

    // on_gallery_tab_changed (Text/Icons tabs)
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_gallery_tab_changed(move |_gallery_tab| {
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                let nav = w.get_active_nav_tab();
                rebuild_gallery(doc, &w, nav);
            }
        });
    }

    // on_element_selected (liste gauche)
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_element_selected(move |kind, idx| {
            if idx < 0 { return; }
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                let (day, night) = render_preview(doc, kind, idx as usize);
                w.set_preview_day(day);
                w.set_preview_night(night);
            }
        });
    }

    // on_gallery_item_selected
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_gallery_item_selected(move |kind, idx| {
            if idx < 0 { return; }
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                let (day, night) = render_preview(doc, kind, idx as usize);
                w.set_preview_day(day);
                w.set_preview_night(night);
            }
        });
    }

    // Stubs Phase 5
    window.on_add_element(|kind| { eprintln!("[stub] add kind={}", kind); });
    window.on_edit_element(|kind, idx| { eprintln!("[stub] edit kind={} idx={}", kind, idx); });
    window.on_delete_element(|kind, idx| { eprintln!("[stub] delete kind={} idx={}", kind, idx); });
    window.on_edit_draworder(|| { eprintln!("[stub] edit draworder"); });

    window.run()?;
    Ok(())
}
