//! Shared Properties-panel layout (#155): the layer header (kind icon + layer name), the one
//! collapsible section header every section uses (separator, disclosure chevron, title), column
//! widths that make field rows fill the panel, and the Quick Actions each layer kind offers.

use egui::{Rect, Sense, pos2, vec2};
use evophoto_doc::{Layer, LayerContent};

use crate::theme::{self, Tokens};
use crate::{EvoPhotoApp, widgets};

/// Height of a section header row.
pub const SECTION_H: f32 = 24.0;
/// Width of the short label (or glyph) column in front of a field: "W", "tT", "VA"…
pub const LABEL_W: f32 = 22.0;
/// Horizontal gap between a label and its field.
pub const LABEL_GAP: f32 = 4.0;
/// Horizontal gap between two label+field columns.
pub const COL_GAP: f32 = 8.0;

/// A one-letter field label ("W", "H", "X", "Y") in a `w`-wide column, ending `pad` before its field. The letters
/// stacked in one column share a centre (#7314): the widest of `column` stays flush right, and the narrower ones
/// are centred on it rather than pushed against its right edge.
pub fn stacked_label(ui: &mut egui::Ui, s: &str, column: &[&str], w: f32, pad: f32) {
    let t = Tokens::get(ui.ctx());
    let font = egui::FontId::proportional(12.0);
    let widest = column.iter().map(|c| ui.painter().layout_no_wrap((*c).to_owned(), font.clone(), t.text_dim).size().x).fold(0.0, f32::max);
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    ui.painter().text(pos2(r.right() - pad - widest / 2.0, r.center().y), egui::Align2::CENTER_CENTER, s, font, t.text_dim);
}

/// Icon for a layer kind (the same glyphs as the Layers panel's kind filter).
pub fn kind_icon(content: &LayerContent) -> &'static str {
    match content {
        LayerContent::Adjustment(_) => "sliders-horizontal",
        LayerContent::Group(_) => "folder",
        LayerContent::Fill(_) => "paint-bucket",
        LayerContent::Text(_) => "type",
        LayerContent::Shape(_) => "pentagon",
        LayerContent::Smart(_) => "app-window",
        LayerContent::Raster(_) => "image",
    }
}

/// "Type Layer", "Curves", "Artboard"…
pub fn kind_label(layer: &Layer) -> String {
    match &layer.content {
        LayerContent::Adjustment(a) => a.label().to_string(),
        LayerContent::Group(g) if g.artboard.is_some() => "Artboard".to_string(),
        other => format!("{} Layer", other.kind_name()),
    }
}

/// Header row: kind icon, the layer's name, and its kind right-aligned in the dim text colour.
pub fn header(ui: &mut egui::Ui, layer: &Layer) {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let icon_r = Rect::from_center_size(pos2(r.left() + 10.0, r.center().y), vec2(20.0, 20.0));
    crate::icons::paint(ui, icon_r, kind_icon(&layer.content), 15.0, t.icon);
    let kind = kind_label(layer);
    let kind_g = ui.painter().layout_no_wrap(tl!(&kind).to_owned(), egui::FontId::proportional(11.0), t.text_faint);
    let kind_w = kind_g.size().x;
    // The name takes what's left between the icon and the kind (elided when long).
    let name_x = r.left() + 26.0;
    let name_w = (r.right() - kind_w - 8.0 - name_x).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(layer.name.clone(), theme::medium(12.5), t.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(name_w);
    let name_g = ui.painter().layout_job(job);
    ui.painter().galley(pos2(name_x, r.center().y - name_g.size().y / 2.0), name_g, t.text);
    if r.width() > kind_w + 60.0 {
        ui.painter().galley(pos2(r.right() - kind_w - 2.0, r.center().y - kind_g.size().y / 2.0), kind_g, t.text_faint);
    }
    let name = layer.name.clone();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("{name} — {kind}")));
}

/// A collapsible section: a separator, then a chevron + title row. Returns whether it's open.
/// Every Properties section uses this, so headers look and behave alike.
pub fn section(ui: &mut egui::Ui, id: &str, title: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    if t.slim() {
        return section_ex(ui, id, title, |_| {});
    }
    let key = egui::Id::new(("props-section", id));
    let mut open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(true);
    ui.add_space(theme::ROW_GAP);
    widgets::hairline(ui);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SECTION_H), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    crate::icons::paint(
        ui,
        Rect::from_center_size(pos2(r.left() + 7.0, r.center().y), vec2(12.0, 12.0)),
        if open { "chevron-down" } else { "chevron-right" },
        11.0,
        t.text_dim,
    );
    ui.painter().text(pos2(r.left() + 18.0, r.center().y), egui::Align2::LEFT_CENTER, tl!(title), theme::semibold(12.0), t.text);
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(key, open));
    }
    let label = title.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, &label));
    record(ui.ctx(), title, r);
    if open {
        ui.add_space(theme::ROW_GAP / 2.0);
    }
    open
}

/// Evora build (patch LUM-18): height of a section header in Evora's graphite grammar.
const SLIM_SECTION_H: f32 = 26.0;

/// Evora build (patch LUM-18): [`section`]'s header in Evora's graphite grammar, which every
/// section takes under the Evora themes: a 26 pt band under a 1 pt structure line, a 12 pt
/// chevron at the start, the title in 12 pt semibold, and `actions` (icon buttons, laid out from
/// the end) at the end of the band. Same open state, accessibility and record as [`section`].
pub fn section_ex(ui: &mut egui::Ui, id: &str, title: &str, actions: impl FnOnce(&mut egui::Ui)) -> bool {
    let t = Tokens::get(ui.ctx());
    let gx = t.graphite();
    let key = egui::Id::new(("props-section", id));
    let mut open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(true);
    ui.add_space(theme::ROW_GAP);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SLIM_SECTION_H), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().rect_filled(r, 0.0, if resp.hovered() { gx.section_hover } else { gx.section });
    // The structure line as the band's own top point row, crisp (a 1 pt stroke centred on a pixel
    // edge spreads over two rows).
    ui.painter().rect_filled(Rect::from_min_max(r.left_top(), pos2(r.right(), r.top() + 1.0)), 0.0, gx.rule);
    // Keyboard focus (WCAG 2.4.7): the bright grey ring, the accent falling under 3:1 on the band.
    if resp.has_focus() {
        widgets::slim_focus_ring(ui, r, &t);
    }
    crate::icons::paint(
        ui,
        Rect::from_center_size(pos2(r.left() + 12.0, r.center().y), vec2(12.0, 12.0)),
        if open { "chevron-down" } else { "chevron-right" },
        12.0,
        t.text_dim,
    );
    ui.painter().text(pos2(r.left() + 24.0, r.center().y), egui::Align2::LEFT_CENTER, tl!(title), theme::semibold(12.0), t.text);
    // Drawn after the band, so its buttons take their own clicks.
    let mut actions_ui = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink2(vec2(4.0, 1.0))).layout(egui::Layout::right_to_left(egui::Align::Center)));
    actions(&mut actions_ui);
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(key, open));
    }
    let label = title.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, &label));
    record(ui.ctx(), title, r);
    if open {
        ui.add_space(theme::ROW_GAP);
    }
    open
}

/// Section headers drawn this pass, top to bottom (title, rect), for tests and `ui.inspect`.
pub fn sections_drawn(ctx: &egui::Context) -> Vec<(String, Rect)> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, Vec<(String, Rect)>)>(egui::Id::new("props-sections"))).filter(|(p, _)| *p + 1 >= pass).map(|(_, v)| v).unwrap_or_default()
}

fn record(ctx: &egui::Context, title: &str, r: Rect) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let e = d.get_temp_mut_or_default::<(u64, Vec<(String, Rect)>)>(egui::Id::new("props-sections"));
        if e.0 != pass {
            *e = (pass, Vec::new());
        }
        e.1.push((title.to_string(), r));
    });
}

/// Width of each field when `cols` label+field pairs (labels `label_w` wide) share the row.
pub fn field_width(avail: f32, cols: usize, label_w: f32) -> f32 {
    let n = cols.max(1) as f32;
    ((avail - n * (label_w + LABEL_GAP) - (n - 1.0) * COL_GAP) / n).max(36.0)
}

/// The Quick Actions a layer kind offers, as (label, command id), Photoshop-style: pixel layers get
/// subject selection, type layers its conversions, shapes and smart objects their own.
pub fn quick_actions(content: &LayerContent) -> &'static [(&'static str, &'static str)] {
    match content {
        LayerContent::Raster(_) => &[("Remove Background", "layer.removeBackground"), ("Select Subject", "select.subject")],
        LayerContent::Text(_) => &[
            ("Convert to Shape", "type.convertToShape"),
            ("Convert to Paragraph Text", "type.convertToParagraphText"),
            ("Convert to Point Text", "type.convertToPointText"),
            ("Create Warped Text", "type.warpText"),
            ("Rasterize Type", "type.rasterizeTypeLayer"),
        ],
        LayerContent::Shape(_) => &[("Rasterize Shape", "layer.rasterize.shape"), ("Convert to Smart Object", "layer.smartObjects.convertToSmartObject")],
        LayerContent::Smart(_) => &[
            ("Edit Contents", "layer.smartObjects.editContents"),
            ("Convert to Layers", "layer.smartObjects.convertToLayers"),
            ("Rasterize Smart Object", "layer.rasterize.smartObject"),
        ],
        LayerContent::Group(_) => &[("Ungroup Layers", "layer.ungroupLayers"), ("Convert to Smart Object", "layer.smartObjects.convertToSmartObject")],
        LayerContent::Fill(_) => &[("Rasterize Fill Content", "layer.rasterize.fillContent")],
        LayerContent::Adjustment(_) => &[],
    }
}

/// The Quick Actions shown right now: the kind's list, limited to commands that exist and are
/// enabled (Convert to Paragraph Text only for point type, and vice versa).
pub fn visible_quick_actions(app: &EvoPhotoApp, content: &LayerContent) -> Vec<(&'static str, &'static str)> {
    quick_actions(content).iter().copied().filter(|(_, id)| crate::menus::is_live(id) && crate::menus::is_enabled(app, id)).collect()
}

/// Quick Actions section body: full-width buttons, two per row when the panel is wide enough.
/// Returns the command to run (through the menus, so ones with dialogs open them).
pub fn quick_actions_ui(app: &EvoPhotoApp, ui: &mut egui::Ui, content: &LayerContent) -> Option<&'static str> {
    let actions = visible_quick_actions(app, content);
    if actions.is_empty() || !section(ui, "quick", "Quick Actions") {
        return None;
    }
    let avail = ui.available_width();
    let cols = if avail >= 360.0 { 2 } else { 1 };
    let mut out = None;
    for row in actions.chunks(cols) {
        // A short last row (or a lone action) still spans the panel.
        let w = ((avail - (row.len() - 1) as f32 * COL_GAP) / row.len() as f32).floor();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = COL_GAP;
            for (label, id) in row {
                let resp = widgets::secondary_button(ui, label, w);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, *label));
                if resp.clicked() {
                    out = Some(*id);
                }
            }
        });
        ui.add_space(theme::ROW_GAP);
    }
    out
}

#[cfg(test)]
#[path = "props_layout_tests.rs"]
mod tests;
