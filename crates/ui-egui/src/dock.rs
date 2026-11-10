//! Right-dock layout (#88): panel groups with fixed heights, like Photoshop's dock columns.
//!
//! A group's height never follows its content: content taller than the group scrolls inside
//! it. The last expanded group (Layers by default) fills what the others leave. Drag the gap
//! between two groups to resize them, double-click a tab (or use the panel menu) to collapse a
//! group to its tab strip, drag a tab strip (outside its tabs, or a tab dragged off the strip) to
//! move the group up or down the column, and drag a tab along its strip to reorder the group's
//! tabs (#2272), or drop a tab on another group's strip to move only that tab. Window › Workspace › Lock Workspace stops all these moves.
//!
//! The layout is [`DockLayout`] in `UiState::dock` (serialisable, drivable with `ui.set`), saved
//! with Window › Workspace › New Workspace…, reset by Reset Workspace, and remembered across
//! launches in the preferences (`panelLayout`) while Remember Workspace Changes is on.

use std::collections::BTreeMap;

use egui::{Pos2, Rect, Sense, Stroke, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::EvoPhotoApp;
use crate::state::{DockTabs, Panels};
use crate::theme::Tokens;
use crate::widgets;

/// A dock panel group (one tab strip).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    /// Color | Swatches | Gradients | Patterns.
    Color,
    /// Properties | Adjustments.
    Properties,
    /// Character | Paragraph (not in the default workspace; Window › Character opens it, #150).
    Character,
    /// Navigator | Histogram | Info.
    Navigator,
    /// History | Actions | Layer Comps.
    History,
    /// Layers | Channels | Paths.
    Layers,
    /// Brushes presets.
    Brushes,
}

impl Group {
    /// Photoshop Essentials order, top to bottom.
    pub const ALL: [Group; 7] = [Group::Color, Group::Properties, Group::Character, Group::Navigator, Group::History, Group::Layers, Group::Brushes];

    pub fn key(self) -> &'static str {
        match self {
            Group::Color => "color",
            Group::Properties => "properties",
            Group::Character => "character",
            Group::Navigator => "navigator",
            Group::History => "history",
            Group::Layers => "layers",
            Group::Brushes => "brushes",
        }
    }

    /// Height (points, tab strip included) a group gets until the user resizes it, when the
    /// column has room (see [`DockLayout::heights_for`]).
    pub fn default_height(self) -> f32 {
        match self {
            Group::Color => 190.0,
            Group::Properties => 250.0,
            Group::Character => 270.0,
            Group::Navigator => 210.0,
            Group::History => 200.0,
            Group::Layers => 320.0,
            Group::Brushes => 200.0,
        }
    }

    /// What a group left at its default height gives way down to so the filler (Layers) keeps
    /// [`Group::preferred_fill`] in a short column (#147). Content taller than this scrolls.
    pub fn compact_height(self) -> f32 {
        match self {
            Group::Color => 130.0,
            Group::Properties => 160.0,
            Group::Character => 160.0,
            Group::Navigator => 140.0,
            Group::History => 130.0,
            Group::Layers => 200.0,
            Group::Brushes => 150.0,
        }
    }

    /// The height the filling group asks for before default-sized groups above it get their
    /// full defaults: Layers wants room for about ten rows at 900 pt (#147).
    pub fn preferred_fill(self) -> f32 {
        match self {
            Group::Layers => 500.0,
            Group::Brushes => 100.0,
            g => g.min_height(),
        }
    }

    /// The smallest height an expanded group can be dragged or squeezed to.
    pub fn min_height(self) -> f32 {
        match self {
            Group::Layers => 140.0,
            Group::Brushes => 100.0,
            _ => 80.0,
        }
    }

    /// Evora build (patch LUM-18): the height (points, tab strip included) under which a group is
    /// not usable. Under the Evora themes an expanded group gets at least this much, and a group
    /// the column has no such room for is folded to its tab strip instead ([`fold_to_fit`]).
    pub fn comfort_height(self) -> f32 {
        match self {
            Group::Layers => 250.0,
            Group::Properties => 200.0,
            Group::Character => 220.0,
            Group::Color => 180.0,
            Group::Navigator => 170.0,
            Group::Brushes => 190.0,
            Group::History => 160.0,
        }
    }

    /// Evora build (patch LUM-18): which group folds first for want of room among those the
    /// member has not asked for: the higher, the sooner. Layers folds last.
    fn fold_rank(self) -> usize {
        match self {
            Group::Navigator => 6,
            Group::Color => 5,
            Group::Brushes => 4,
            Group::Character => 3,
            Group::History => 2,
            Group::Properties => 1,
            Group::Layers => 0,
        }
    }

    pub fn tabs(self, pro: bool) -> &'static [&'static str] {
        match self {
            // Photoshop Essentials: Color | Swatches | Gradients | Patterns.
            Group::Color if pro => &["Color", "Swatches", "Gradients", "Patterns"],
            Group::Color => &["Swatches", "Color", "Gradients", "Patterns"],
            Group::Properties => &["Properties", "Adjustments"],
            Group::Character => &["Character", "Paragraph"],
            Group::Navigator => &["Navigator", "Histogram", "Info"],
            Group::History => &["History", "Actions", "Layer Comps"],
            Group::Layers => &["Layers", "Channels", "Paths"],
            Group::Brushes => &["Brushes"],
        }
    }

    /// Tabs that lay out their own scrolling list and footer, or size themselves to the group
    /// (the Navigator's preview grows with it, as in Photoshop: #2726). Channels and Paths are
    /// Layers' siblings and do the same: wrapped in the dock's scroll area as well, the two nested
    /// areas shared the wheel, and where the outer content overflowed by one physical pixel (at
    /// 125 % scaling for some window heights) the footer and list moved a pixel as you scrolled (#6238).
    pub fn scrolls_itself(self, tab: usize) -> bool {
        matches!((self, tab), (Group::Layers, 0..=2) | (Group::History, 0) | (Group::History, 1) | (Group::Navigator, 0))
    }

    pub(crate) fn tab_mut(self, tabs: &mut DockTabs) -> &mut usize {
        match self {
            Group::Color => &mut tabs.color,
            Group::Properties => &mut tabs.properties,
            Group::Character => &mut tabs.character,
            Group::Navigator => &mut tabs.navigator,
            Group::History => &mut tabs.history,
            Group::Layers => &mut tabs.layers,
            Group::Brushes => &mut tabs.brushes,
        }
    }

    /// The group for a `panels` / `dockTabs` key ("color", "properties", …).
    pub fn from_key(key: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.key() == key)
    }

    pub fn shown(self, panels: &Panels) -> bool {
        match self {
            Group::Color => panels.color,
            Group::Properties => panels.properties,
            Group::Character => panels.character,
            Group::Navigator => panels.navigator,
            Group::History => panels.history,
            Group::Layers => panels.layers,
            Group::Brushes => panels.brushes,
        }
    }

    pub(crate) fn shown_mut(self, panels: &mut Panels) -> &mut bool {
        match self {
            Group::Color => &mut panels.color,
            Group::Properties => &mut panels.properties,
            Group::Character => &mut panels.character,
            Group::Navigator => &mut panels.navigator,
            Group::History => &mut panels.history,
            Group::Layers => &mut panels.layers,
            Group::Brushes => &mut panels.brushes,
        }
    }
}

/// Order, heights, collapsed state and visibility of individual dock tabs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DockLayout {
    /// Top-to-bottom order; a group missing here (say one added after the layout was saved)
    /// goes just below the nearest group that precedes it by default (or last).
    pub order: Vec<Group>,
    /// Heights the user dragged groups to (points, tab strip included). Unset = default.
    pub heights: BTreeMap<Group, f32>,
    /// Groups collapsed to their tab strip.
    pub collapsed: Vec<Group>,
    /// Names of tabs hidden by Close (not positions: Color/Swatches swap with the theme).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub hidden_tabs: BTreeMap<Group, Vec<String>>,
    /// Tab order per group, left to right, by name like `hidden_tabs` (#2272). A group missing
    /// here keeps its default order; unknown names are ignored and tabs missing from a list
    /// follow it in their default order. Anything unreadable falls back to the defaults.
    #[serde(rename = "tabOrder", alias = "tab_order", deserialize_with = "lenient_tab_order")]
    pub tab_order: BTreeMap<Group, Vec<String>>,
}

/// Read `tabOrder` keeping what makes sense: unknown groups, non-list values and non-string names
/// are dropped instead of failing the whole saved layout.
fn lenient_tab_order<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<Group, Vec<String>>, D::Error> {
    let v = Value::deserialize(d)?;
    let mut out = BTreeMap::new();
    for (k, names) in v.as_object().into_iter().flatten() {
        let (Some(g), Some(names)) = (Group::from_key(k), names.as_array()) else { continue };
        out.insert(g, names.iter().filter_map(Value::as_str).take(16).map(str::to_owned).collect());
    }
    Ok(out)
}

/// Gap between groups; it is also the splitter's grab area.
pub const GAP: f32 = 6.0;
/// Upper bound on a stored height (guards against absurd values from `ui.set`).
const MAX_HEIGHT: f32 = 4000.0;

impl DockLayout {
    /// Every tab of `group` in the user's order (#2272), with its original index (the one
    /// `dockTabs`, commands and panel bodies use).
    pub fn ordered_tabs(&self, group: Group, pro: bool) -> Vec<(usize, &'static str)> {
        let all: Vec<(usize, &'static str)> = group.tabs(pro).iter().copied().enumerate().collect();
        let mut out: Vec<(usize, &'static str)> = Vec::with_capacity(all.len());
        for name in self.tab_order.get(&group).into_iter().flatten() {
            if let Some(&tab) = all.iter().find(|(_, n)| *n == name.as_str())
                && !out.contains(&tab)
            {
                out.push(tab);
            }
        }
        for tab in all {
            if !out.contains(&tab) {
                out.push(tab);
            }
        }
        out
    }

    /// Visible tabs in display order, retaining their original indices for commands and panel
    /// bodies.
    pub fn visible_tabs(&self, group: Group, pro: bool) -> Vec<(usize, &'static str)> {
        let hidden = self.hidden_tabs.get(&group);
        self.ordered_tabs(group, pro).into_iter().filter(|(_, name)| !hidden.is_some_and(|xs| xs.iter().any(|x| x.as_str() == *name))).collect()
    }

    /// Move tab `tab` (an original index) of `group` so it is drawn just before tab `before`
    /// (or last when `None` or not a tab of the group). The default order is stored as no entry.
    pub fn move_tab(&mut self, group: Group, tab: usize, before: Option<usize>, pro: bool) {
        if before == Some(tab) {
            return;
        }
        let mut order = self.ordered_tabs(group, pro);
        let Some(at) = order.iter().position(|(i, _)| *i == tab) else { return };
        let moved = order.remove(at);
        let to = before.and_then(|b| order.iter().position(|(i, _)| *i == b)).unwrap_or(order.len());
        order.insert(to, moved);
        if order.iter().map(|(i, _)| *i).eq(0..order.len()) {
            self.tab_order.remove(&group);
        } else {
            self.tab_order.insert(group, order.into_iter().map(|(_, n)| n.to_owned()).collect());
        }
    }

    /// Hide one tab; it can be reopened through Window › Panel.
    pub fn hide_tab(&mut self, group: Group, tab: usize, pro: bool) {
        let Some(&name) = group.tabs(pro).get(tab) else { return };
        let hidden = self.hidden_tabs.entry(group).or_default();
        if !hidden.iter().any(|x| x.as_str() == name) {
            hidden.push(name.to_owned());
        }
    }

    /// Restore the tab requested by the Window menu or icon rail.
    pub fn show_tab(&mut self, group: Group, tab: usize, pro: bool) {
        let Some(&name) = group.tabs(pro).get(tab) else { return };
        if let Some(hidden) = self.hidden_tabs.get_mut(&group) {
            hidden.retain(|n| n != name);
            if hidden.is_empty() {
                self.hidden_tabs.remove(&group);
            }
        }
    }

    /// Every group once, in display order.
    pub fn order(&self) -> Vec<Group> {
        let mut out: Vec<Group> = Vec::with_capacity(Group::ALL.len());
        for g in &self.order {
            if !out.contains(g) {
                out.push(*g);
            }
        }
        // Missing groups slot in below their default predecessor, so a group new to an old
        // saved layout (Character) never lands below Layers and takes over as the filler.
        for (i, g) in Group::ALL.iter().enumerate() {
            if out.contains(g) {
                continue;
            }
            let prev = Group::ALL.iter().take(i).rev().find_map(|p| out.iter().position(|x| x == p));
            match prev {
                Some(at) => out.insert(at + 1, *g),
                None => out.push(*g),
            }
        }
        out
    }

    pub fn is_collapsed(&self, g: Group) -> bool {
        self.collapsed.contains(&g)
    }

    pub fn set_collapsed(&mut self, g: Group, on: bool) {
        self.collapsed.retain(|c| *c != g);
        if on {
            self.collapsed.push(g);
        }
    }

    /// The stored (or default) height, sanitised.
    pub fn height(&self, g: Group) -> f32 {
        match self.heights.get(&g) {
            Some(h) if h.is_finite() => h.clamp(g.min_height(), MAX_HEIGHT),
            _ => g.default_height(),
        }
    }

    /// Move `g` so it is drawn just before `before` (or last when `None`).
    pub fn move_group(&mut self, g: Group, before: Option<Group>) {
        if before == Some(g) {
            return;
        }
        let mut order = self.order();
        order.retain(|x| *x != g);
        let at = before.and_then(|b| order.iter().position(|x| *x == b)).unwrap_or(order.len());
        order.insert(at, g);
        self.order = order;
    }

    /// Lay out the `shown` groups (in display order) in a column `avail` points tall with
    /// `strip`-high tab strips. Returns each group's height. The last expanded group fills the
    /// rest. Groups the user never resized give way first, down to their compact heights, so
    /// the filler gets its preferred height (Layers: ~10 rows, #147); when the column is still
    /// too short every group gives way down to its minimum height.
    pub fn heights_for(&self, shown: &[Group], avail: f32, strip: f32) -> Vec<(Group, f32)> {
        self.heights_fit(shown, avail, strip, &[], false)
    }

    /// Evora build (patch LUM-18): [`DockLayout::heights_for`] with the groups in `folded` drawn
    /// as their tab strip ([`fold_to_fit`]) and, with `comfort` (the Evora themes), every expanded
    /// group kept at [`Group::comfort_height`] or more: it is the floor both passes give way down
    /// to, and the filler takes what is left instead of running past the column when even its
    /// comfort does not fit (its body scrolls). Without `folded` and `comfort` this is upstream's
    /// layout, unchanged.
    pub fn heights_fit(&self, shown: &[Group], avail: f32, strip: f32, folded: &[Group], comfort: bool) -> Vec<(Group, f32)> {
        let avail = if avail.is_finite() { avail.max(0.0) } else { 0.0 };
        let closed = |g: Group| self.is_collapsed(g) || folded.contains(&g);
        let filler = shown.iter().rposition(|g| !closed(*g));
        let mut hs: Vec<f32> = shown
            .iter()
            .enumerate()
            .map(|(i, g)| {
                if closed(*g) {
                    strip
                } else if Some(i) == filler {
                    0.0
                } else if comfort {
                    self.height(*g).max(g.comfort_height())
                } else {
                    self.height(*g)
                }
            })
            .collect();
        if let Some(f) = filler {
            let gaps = GAP * shown.len().saturating_sub(1) as f32;
            let min_fill = shown.get(f).map_or(0.0, |g| if comfort { g.comfort_height() } else { g.min_height() });
            let pref_fill = shown.get(f).map_or(0.0, |g| if comfort { g.preferred_fill().max(g.comfort_height()) } else { g.preferred_fill() });
            let used: f32 = hs.iter().sum::<f32>() + gaps;
            let mut deficit = (used + pref_fill - avail).max(0.0);
            for i in (0..f).rev() {
                if deficit <= 0.0 {
                    break;
                }
                let Some(g) = shown.get(i) else { continue };
                if closed(*g) || self.heights.contains_key(g) {
                    continue;
                }
                if let Some(h) = hs.get_mut(i) {
                    let floor = if comfort { g.comfort_height() } else { g.compact_height() };
                    let give = (*h - floor).max(0.0).min(deficit);
                    *h -= give;
                    deficit -= give;
                }
            }
            let used: f32 = hs.iter().sum::<f32>() + gaps;
            let mut deficit = (used + min_fill - avail).max(0.0);
            // Squeeze the expanded groups nearest the filler first.
            for i in (0..f).rev() {
                if deficit <= 0.0 {
                    break;
                }
                let Some(g) = shown.get(i) else { continue };
                if closed(*g) {
                    continue;
                }
                if let Some(h) = hs.get_mut(i) {
                    let floor = if comfort { g.comfort_height() } else { g.min_height() };
                    let give = (*h - floor).max(0.0).min(deficit);
                    *h -= give;
                    deficit -= give;
                }
            }
            let rest = avail - hs.iter().sum::<f32>() - gaps;
            if let Some(h) = hs.get_mut(f) {
                *h = if comfort { rest.max(0.0) } else { rest.max(min_fill) };
            }
        }
        shown.iter().copied().zip(hs).collect()
    }
}

/// Evora build (patch LUM-18): what the dock derives each frame under the Evora themes, kept in
/// memory only (`UiState::dock_fit`, never serialised, never part of a saved layout).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockFit {
    /// The groups the member asked for (rail, Window menu, a strip or a tab clicked, a group
    /// expanded), most recent first: the last ones to fold when the column is too short.
    pub recent: Vec<Group>,
    /// The groups folded to their tab strip for want of room when the dock was last drawn
    /// ([`fold_to_fit`]). Never written to [`DockLayout::collapsed`]: they come back on their own
    /// when the column grows.
    pub folded: Vec<Group>,
    /// The revision of Evora's layout the remembered layout was made under (`evoraLayout` in
    /// `panelLayout`; 0 for a layout from before revisions, or from upstream). See
    /// `menus::adopt_evora_workspace`.
    pub revision: u32,
    /// The remembered layout was checked against Evora's revision this session (once, the first
    /// time an Evora theme draws the dock).
    pub adopted: bool,
}

/// Evora build (patch LUM-18): "a panel is usable or folded, never squeezed". The groups of
/// `shown` (display order) to draw as their tab strip because a column `avail` points tall cannot
/// give every expanded group its [`Group::comfort_height`]: while the comforts of the expanded
/// groups, the `strip`-high strips of the others and the `gap`s between groups do not fit, and
/// more than one group is expanded, the group asked for least recently folds. The comforts alone
/// count: a height the member dragged a group to gives way down to the comfort in
/// [`DockLayout::heights_fit`] and never folds a group the column has room for, so what is folded
/// depends on the column and on what was asked for, not on a splitter. `recent` lists the groups
/// the member asked for, most recent first; the groups missing from it fold before those,
/// Navigator first and Layers last. `collapsed` (the member's own collapsed groups) is
/// [`DockLayout`]'s. The last expanded group never folds. Pure: nothing is stored.
pub fn fold_to_fit(shown: &[Group], collapsed: &[Group], avail: f32, strip: f32, gap: f32, recent: &[Group]) -> Vec<Group> {
    let avail = if avail.is_finite() { avail.max(0.0) } else { 0.0 };
    let gaps = gap * shown.len().saturating_sub(1) as f32;
    let mut folded: Vec<Group> = Vec::new();
    loop {
        let open: Vec<Group> = shown.iter().copied().filter(|g| !collapsed.contains(g) && !folded.contains(g)).collect();
        if open.len() < 2 {
            break;
        }
        let wanted: f32 = shown.iter().map(|g| if open.contains(g) { g.comfort_height() } else { strip }).sum();
        // Half a point of slack for the rounding of the column's height.
        if wanted + gaps <= avail + 0.5 {
            break;
        }
        let rank = |g: &Group| recent.iter().position(|r| r == g).unwrap_or(recent.len() + 1 + g.fold_rank());
        let Some(victim) = open.iter().copied().max_by_key(rank) else { break };
        folded.push(victim);
    }
    folded
}

/// Evora build (patch LUM-18): whether `g` was drawn folded for want of room by the native dock
/// under an Evora theme ([`fold_to_fit`]). Always false in the custom dock and upstream's themes.
pub fn is_folded(ui: &crate::state::UiState, g: Group) -> bool {
    ui.theme.is_slim() && ui.docking.is_none() && ui.dock_fit.folded.contains(&g)
}

/// Evora build (patch LUM-18): the member just asked for `g`: it goes first among the groups kept
/// open when the column is too short ([`DockFit::recent`]). Nothing under upstream's themes.
pub(crate) fn touch_recent(app: &mut EvoPhotoApp, g: Group) {
    if !app.ui.theme.is_slim() {
        return;
    }
    let recent = &mut app.ui.dock_fit.recent;
    recent.retain(|r| *r != g);
    recent.insert(0, g);
}

/// Show `g` and expand it (Window › <panel>, the icon rail): a panel asked for is always
/// brought back, whatever state it was left in (#129).
pub fn reveal(app: &mut EvoPhotoApp, g: Group) {
    *g.shown_mut(&mut app.ui.panels) = true;
    let selected = *g.tab_mut(&mut app.ui.dock_tabs);
    let pro = app.ui.theme.is_pro();
    app.ui.dock.show_tab(g, selected, pro);
    app.ui.dock.set_collapsed(g, false);
    touch_recent(app, g);
}

/// Icon rail click: a hidden group is shown, a collapsed one expanded and an expanded one
/// collapsed to its tab strip. A docked group is never hidden from the rail (it used to
/// toggle visibility, so one stray click made a panel vanish: #129); `docked` is false for
/// Studio's floating Properties card, which the rail shows and hides.
/// Evora build (patch LUM-18): a group folded for want of room ([`fold_to_fit`]) is asked for
/// instead: it comes back and the group asked for least recently folds in its place.
pub fn rail_click(app: &mut EvoPhotoApp, g: Group, docked: bool) {
    if !g.shown(&app.ui.panels) {
        reveal(app, g);
    } else if !docked {
        *g.shown_mut(&mut app.ui.panels) = false;
    } else if is_folded(&app.ui, g) {
        touch_recent(app, g);
    } else {
        let collapse = !app.ui.dock.is_collapsed(g);
        app.ui.dock.set_collapsed(g, collapse);
        if !collapse {
            touch_recent(app, g);
        }
    }
}

/// A tab strip's Close: hide `tab` of `g`, and the group with its last tab. When the selected
/// tab closes, the next one on the strip (or the previous one, for the last) is selected.
pub(crate) fn close_tab(app: &mut EvoPhotoApp, g: Group, tab: usize, pro: bool) {
    let shown_before = app.ui.dock.visible_tabs(g, pro);
    app.ui.dock.hide_tab(g, tab, pro);
    let remaining = app.ui.dock.visible_tabs(g, pro);
    if remaining.is_empty() {
        *g.shown_mut(&mut app.ui.panels) = false;
    } else if *g.tab_mut(&mut app.ui.dock_tabs) == tab {
        // Prefer the next tab on the strip, or the previous one when closing the last.
        let at = shown_before.iter().position(|(i, _)| *i == tab).unwrap_or(0);
        let next = shown_before.iter().skip(at + 1).find(|x| remaining.contains(x)).or_else(|| remaining.last());
        if let Some(&(next, _)) = next {
            *g.tab_mut(&mut app.ui.dock_tabs) = next;
        }
    }
}

/// Per-group interactions collected while drawing, applied afterwards.
enum Action {
    ToggleCollapse(Group),
    Close(Group),
    CloseTab(Group, usize),
    Move(Group, Option<Group>),
    MoveTab(Group, usize, Option<usize>),
}

/// Rects of the groups drawn last frame (screen points), for tests and automation.
pub fn last_rects(ctx: &egui::Context) -> Vec<(Group, Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(Group, Rect)>>(rects_id())).unwrap_or_default()
}

fn rects_id() -> egui::Id {
    egui::Id::new("dock-group-rects")
}

/// The group expanded from its collapsed tab strip (see [`show`]), if any: with Preferences ▸
/// Workspace ▸ Auto-Collapse Iconic Panels on, the first click outside it collapses it again.
fn peek_id() -> egui::Id {
    egui::Id::new("dock-peek")
}

/// Evora build (patch LUM-18): the group a click just brought back from its fold, and when
/// (`InputState::time`), so the second click of a double-click does not collapse it.
fn recall_id() -> egui::Id {
    egui::Id::new("dock-recall")
}

/// Evora build (patch LUM-18): how long (seconds) after such a click a double-click is ignored.
const RECALL_GRACE: f64 = 0.5;

fn auto_collapse_peek(app: &mut EvoPhotoApp, ctx: &egui::Context) {
    let Some(g) = ctx.data(|d| d.get_temp::<Option<Group>>(peek_id())).flatten() else { return };
    let Some(p) = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()).flatten()) else {
        return;
    };
    let inside = last_rects(ctx).into_iter().find(|(x, _)| *x == g).is_some_and(|(_, r)| r.contains(p));
    if !inside {
        app.ui.dock.set_collapsed(g, true);
        ctx.data_mut(|d| {
            d.insert_temp(peek_id(), None::<Group>);
        });
    }
}

/// A group's tab strip as drawn last frame (screen points), for tests and automation.
#[derive(Clone, Debug, PartialEq)]
pub struct StripRects {
    pub group: Group,
    /// The area accepting a tab drop, excluding the group menu.
    pub area: Rect,
    /// `(tab index, rect)` of the tabs on the strip (the others are in the chevron menu).
    pub tabs: Vec<(usize, Rect)>,
    /// The panel menu button.
    pub menu: Rect,
    /// The » overflow button, when some tabs didn't fit.
    pub chevron: Option<Rect>,
}

/// The tab strips drawn last frame.
pub fn last_strips(ctx: &egui::Context) -> Vec<StripRects> {
    ctx.data(|d| d.get_temp::<Vec<StripRects>>(strips_id())).unwrap_or_default()
}

fn strips_id() -> egui::Id {
    egui::Id::new("dock-strip-rects")
}

/// Draw the `shown` groups (any order; the layout decides) filling `ui`. `body` draws one
/// group's tab content.
pub fn show(app: &mut EvoPhotoApp, ui: &mut egui::Ui, shown: &[Group], mut body: impl FnMut(&mut EvoPhotoApp, &mut egui::Ui, Group, usize)) {
    let t = Tokens::get(ui.ctx());
    let auto_collapse = app.session.prefs().workspace.auto_collapse_icon_panels;
    if auto_collapse {
        auto_collapse_peek(app, ui.ctx());
    } else {
        ui.ctx().data_mut(|d| {
            d.insert_temp(peek_id(), None::<Group>);
        });
    }
    let strip = if t.pro { 28.0 } else { 40.0 };
    let order: Vec<Group> = app.ui.dock.order().into_iter().filter(|g| shown.contains(g)).collect();
    let area = ui.available_rect_before_wrap();
    // Evora build (patch LUM-18): under the Evora themes a group is usable or folded, never
    // squeezed. The groups the column has no room for are drawn as their strip this frame; the
    // set is derived, never written to the layout, so they come back when the column grows.
    let comfort = t.slim();
    let folded: Vec<Group> = if comfort {
        fold_to_fit(&order, &app.ui.dock.collapsed, area.height(), strip, GAP, &app.ui.dock_fit.recent)
    } else {
        Vec::new()
    };
    if app.ui.dock_fit.folded != folded {
        app.ui.dock_fit.folded = folded.clone();
    }
    let now = ui.input(|i| i.time);
    let recalled_before = if comfort { ui.ctx().data(|d| d.get_temp::<(Group, f64)>(recall_id())) } else { None };
    let mut asked: Vec<Group> = Vec::new();
    let mut recalled: Option<Group> = None;
    // Evora build (patch LUM-18): where the pointer was just pressed, if it was. A press inside an
    // expanded group asks for it as a click on its strip does: the panel the member works in is
    // not the one that folds when another is brought in.
    let pressed_at: Option<Pos2> = if comfort { ui.input(|i| i.pointer.any_pressed().then(|| i.pointer.press_origin()).flatten()) } else { None };
    let heights = app.ui.dock.heights_fit(&order, area.height(), strip, &folded, comfort);
    let locked = app.session.prefs().workspace_locked;
    let rects = rects_after_layout(&heights, area);
    let mut actions: Vec<Action> = Vec::new();
    let mut peek_after: Option<Group> = None;
    let mut tab_drops: Vec<(Group, usize, Pos2)> = Vec::new();
    let mut dragging: Option<Group> = None;
    let mut strips: Vec<StripRects> = Vec::with_capacity(rects.len());
    for (i, (g, rect)) in rects.iter().copied().enumerate() {
        let member_collapsed = app.ui.dock.is_collapsed(g);
        let auto_folded = folded.contains(&g);
        let collapsed = member_collapsed || auto_folded;
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("dock-group", g.key())).max_rect(rect));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing.y = if t.pro { 0.0 } else { 6.0 };
        let before = *g.tab_mut(&mut app.ui.dock_tabs);
        let visible = app.ui.dock.visible_tabs(g, t.pro);
        if visible.is_empty() {
            continue;
        }
        // Evora build (patch LUM-18): a press in this expanded group, on the dock's own layer (not
        // on a menu or a dialog drawn over it), asks for it.
        if !collapsed && pressed_at.is_some_and(|p| rect.contains(p) && ui.ctx().layer_id_at(p) == Some(ui.layer_id())) {
            asked.push(g);
        }
        let indices: Vec<usize> = visible.iter().map(|(i, _)| *i).collect();
        let tabs: Vec<&str> = visible.iter().map(|(_, name)| *name).collect();
        let mut sel = indices.iter().position(|i| *i == before).unwrap_or(0);
        let resp = widgets::card_ex(&mut child, g.key(), &tabs, &mut sel, collapsed, !locked, |ui, shown_tab| {
            let Some(&tab) = indices.get(shown_tab) else { return };
            let inner = ui.available_height().max(0.0);
            if g.scrolls_itself(tab) {
                ui.set_min_height(inner);
                body(app, ui, g, tab);
            } else {
                crate::panels::offer_color_field_fill(ui, t.pro, g, tab);
                // No scroll fade: it darkens real colours, swatches and previews at the edge
                // (#2726); the scroll bar already shows there is more.
                ui.spacing_mut().scroll.fade.strength = 0.0;
                egui::ScrollArea::vertical()
                    .id_salt(("dock-scroll", g.key(), tab))
                    .max_height(inner)
                    .auto_shrink([false, false])
                    .show(ui, |ui| body(app, ui, g, tab));
            }
        });
        for (index, response) in &resp.responses {
            if let Some(panel) = tabs.get(*index) {
                crate::panel_docking::legacy_tab(app, &mut child, panel, response);
            }
        }
        strips.push(StripRects {
            group: g,
            area: Rect::from_min_max(pos2(rect.left(), resp.menu.rect.top()), pos2(resp.menu.rect.left(), resp.menu.rect.bottom())),
            tabs: resp.tabs.iter().filter_map(|(i, r)| indices.get(*i).map(|original| (*original, *r))).collect(),
            menu: resp.menu.rect,
            chevron: resp.chevron,
        });
        // The strip uses visible indices; dockTabs and the panel bodies use original indices.
        if let Some(&picked) = indices.get(sel)
            && picked != before
        {
            *g.tab_mut(&mut app.ui.dock_tabs) = picked;
        }
        if let Some(context) = resp.tab_context {
            match context {
                crate::tab_strip::TabContextAction::Close(i) => {
                    if let Some(&tab) = indices.get(i) {
                        actions.push(Action::CloseTab(g, tab));
                    }
                }
                crate::tab_strip::TabContextAction::CloseGroup => actions.push(Action::Close(g)),
            }
        }
        // Evora build (patch LUM-18): a click on a strip or a tab asks for the group. One folded
        // for want of room comes back on that click (another folds in its place) and its
        // collapsed state is left alone; the second click of a double-click does not collapse it.
        if comfort && (resp.tab_clicked || resp.strip.clicked()) {
            asked.push(g);
            if auto_folded {
                recalled = Some(g);
            }
        }
        let just_recalled = recalled_before.is_some_and(|(r, at)| r == g && now - at < RECALL_GRACE);
        if !auto_folded && !just_recalled && (resp.strip.double_clicked() || resp.tab_double_clicked || (collapsed && resp.tab_clicked)) {
            actions.push(Action::ToggleCollapse(g));
            // A single click on a collapsed strip's tab expands the group as a peek: the next
            // click outside closes it again, with Auto-Collapse Iconic Panels on.
            if collapsed && resp.tab_clicked && !resp.tab_double_clicked && auto_collapse {
                peek_after = Some(g);
            }
        }
        // The strip uses visible indices; reorder in original ones.
        if !locked
            && let Some((from, before)) = resp.tab_reorder
            && let Some(&tab) = indices.get(from)
        {
            actions.push(Action::MoveTab(g, tab, before.and_then(|b| indices.get(b).copied())));
        }
        // A tab dragged off its strip moves the whole group, as the strip itself does.
        if !locked && app.ui.docking.is_none() && (resp.strip.dragged() || resp.tab_dragging_out.is_some()) {
            dragging = Some(g);
        }
        if !locked
            && app.ui.docking.is_none()
            && resp.strip.drag_stopped()
            && let Some(p) = ui.ctx().pointer_interact_pos().or_else(|| ui.ctx().pointer_latest_pos())
        {
            actions.push(Action::Move(g, drop_before(&order, &rects, g, p.y)));
        }
        if !locked
            && app.ui.docking.is_none()
            && let Some(shown_tab) = resp.tab_dropped_out
            // The strip uses visible indices; the drop resolves the panel by its original one.
            && let Some(&tab) = indices.get(shown_tab)
            && let Some(p) = ui.ctx().pointer_interact_pos().or_else(|| ui.ctx().pointer_latest_pos())
        {
            tab_drops.push((g, tab, p));
        }
        if dragging == Some(g) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        egui::Popup::menu(&resp.menu).show(|ui| {
            crate::widgets::style_spectrum_popup_menu(ui);
            ui.set_min_width(170.0);
            if tabs.get(sel) == Some(&"Layers") {
                crate::layer_row_ui::panel_menu(app, ui);
                ui.separator();
            }
            if tabs.get(sel) == Some(&"Color") {
                crate::color_panel_ui::panel_menu(app, ui);
                ui.separator();
            }
            if tabs.get(sel) == Some(&"Swatches") {
                crate::swatches_ui::panel_menu(app, ui);
                ui.separator();
            }
            if tabs.get(sel) == Some(&"History") {
                crate::history_menu_ui::panel_menu(app, ui);
                ui.separator();
            }
            if tabs.get(sel) == Some(&"Brush Settings") {
                crate::brush_panel::panel_menu(app, ui);
                ui.separator();
            }
            if tabs.get(sel) == Some(&"Actions") {
                crate::actions::panel_menu(app, ui);
                ui.separator();
            }
            if let Some(label) = tabs.get(sel) {
                crate::panel_docking::legacy_menu(app, ui, label);
            }
            if ui.button(if collapsed { tl!("Expand Panel Group") } else { tl!("Collapse Panel Group") }).clicked() {
                // Evora build (patch LUM-18): a group folded for want of room is asked for.
                if auto_folded {
                    asked.push(g);
                } else {
                    actions.push(Action::ToggleCollapse(g));
                }
                ui.close();
            }
            let pos = order.iter().position(|x| *x == g).unwrap_or(0);
            if ui.add_enabled(!locked && pos > 0, egui::Button::new(tl!("Move Group Up"))).clicked() {
                actions.push(Action::Move(g, order.get(pos.saturating_sub(1)).copied()));
                ui.close();
            }
            if ui.add_enabled(!locked && pos + 1 < order.len(), egui::Button::new(tl!("Move Group Down"))).clicked() {
                actions.push(Action::Move(g, order.get(pos + 2).copied()));
                ui.close();
            }
            ui.separator();
            if ui.button(tl!("Close")).clicked() {
                if let Some(&tab) = indices.get(sel) {
                    actions.push(Action::CloseTab(g, tab));
                }
                ui.close();
            }
            if ui.button(tl!("Close Tab Group")).clicked() {
                actions.push(Action::Close(g));
                ui.close();
            }
        });
        // Splitter in the gap below this group: resizes it, pushing the neighbouring groups.
        if i + 1 < rects.len() && !collapsed && rects.iter().skip(i + 1).any(|(n, _)| !app.ui.dock.is_collapsed(*n) && !folded.contains(n)) {
            let gap = Rect::from_min_size(pos2(rect.left(), rect.bottom()), vec2(rect.width(), GAP)).expand2(vec2(0.0, 2.0));
            let sresp = ui.interact(gap, ui.id().with(("dock-splitter", g.key())), Sense::drag());
            if sresp.hovered() || sresp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                ui.painter().line_segment([gap.left_center(), gap.right_center()], Stroke::new(2.0, t.accent.gamma_multiply(0.7)));
            }
            if sresp.dragged() {
                resize_fit(&mut app.ui.dock, &heights, i, sresp.drag_delta().y, &folded, comfort);
            }
        }
    }
    let mut panel_moves = Vec::new();
    for (from, tab, position) in tab_drops {
        let target = strips.iter().find(|strip| strip.group != from && strip.area.contains(position));
        if let Some(target) = target {
            let pro = t.pro;
            let source = from.tabs(pro).get(tab).and_then(|label| crate::panel_docking::normalize(label));
            let slot = crate::tab_strip::drop_slot(&target.tabs, target.area, position);
            let before = slot.and_then(|slot| target.tabs.get(slot).map(|(index, _)| *index));
            let anchor = before.or_else(|| target.tabs.first().map(|(index, _)| *index));
            let destination = anchor.and_then(|index| target.group.tabs(pro).get(index).and_then(|label| crate::panel_docking::normalize(label)));
            if let (Some(source), Some(destination)) = (source, destination) {
                panel_moves.push((source, destination, before.is_some()));
            }
        } else {
            actions.push(Action::Move(from, drop_before(&order, &rects, from, position.y)));
        }
    }
    // Drop indicator while a group is dragged by its tab strip.
    if let (Some(g), Some(p)) = (dragging, ui.ctx().pointer_interact_pos()) {
        let before = drop_before(&order, &rects, g, p.y);
        let line_y = match before.and_then(|b| rects.iter().find(|(x, _)| *x == b)) {
            Some((_, r)) => r.top() - GAP / 2.0,
            None => rects.last().map_or(area.top(), |(_, r)| r.bottom() + GAP / 2.0),
        };
        ui.painter().line_segment([pos2(area.left(), line_y), pos2(area.right(), line_y)], Stroke::new(3.0, t.accent));
    }
    ui.ctx().data_mut(|d| {
        d.insert_temp(rects_id(), rects);
        d.insert_temp(strips_id(), strips);
    });
    ui.advance_cursor_after_rect(area);
    for a in actions {
        match a {
            Action::ToggleCollapse(g) => {
                let on = !app.ui.dock.is_collapsed(g);
                app.ui.dock.set_collapsed(g, on);
                if !on {
                    touch_recent(app, g);
                }
            }
            Action::Close(g) => *g.shown_mut(&mut app.ui.panels) = false,
            Action::CloseTab(g, tab) => close_tab(app, g, tab, t.pro),
            Action::MoveTab(g, tab, before) => app.ui.dock.move_tab(g, tab, before, t.pro),
            Action::Move(g, before) => {
                if before != Some(g) {
                    app.ui.dock.move_group(g, before);
                }
            }
        }
    }
    if let Some(g) = peek_after
        && !app.ui.dock.is_collapsed(g)
    {
        ui.ctx().data_mut(|d| {
            d.insert_temp(peek_id(), Some(g));
        });
    }
    // Evora build (patch LUM-18): the groups asked for this frame go first among those kept open.
    for g in asked {
        touch_recent(app, g);
    }
    if let Some(g) = recalled {
        ui.ctx().data_mut(|d| {
            d.insert_temp(recall_id(), (g, now));
        });
    }
    for (panel, anchor, before) in panel_moves {
        let params = if before { json!({"panel":panel,"anchor":anchor,"before":anchor}) } else { json!({"panel":panel,"anchor":anchor}) };
        if let Err(error) = app.run("window.panel.move", params) {
            app.ui.status = error;
        }
    }
}

fn rects_after_layout(heights: &[(Group, f32)], area: Rect) -> Vec<(Group, Rect)> {
    let mut y = area.top();
    heights
        .iter()
        .map(|(g, h)| {
            let r = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), h.max(0.0)));
            y = r.bottom() + GAP;
            (*g, r)
        })
        .collect()
}

/// The group the dragged one lands before when released at `y` (`None` = last).
fn drop_before(order: &[Group], rects: &[(Group, Rect)], dragged: Group, y: f32) -> Option<Group> {
    let hit = rects.iter().find(|(_, r)| y < r.center().y).map(|(g, _)| *g);
    match hit {
        // Dropping onto itself, or just below itself, keeps the place.
        Some(g) if g == dragged => Some(dragged),
        Some(g) => {
            let after_self = order.iter().position(|x| *x == dragged).zip(order.iter().position(|x| *x == g)).is_some_and(|(a, b)| b == a + 1);
            if after_self { Some(dragged) } else { Some(g) }
        }
        None => None,
    }
}

/// Splitter `i` (below group `i`) dragged by `dy`. Like Photoshop, the groups on the side the
/// splitter moves into shrink in turn, nearest first, each down to its minimum (#2573), and the
/// group on the other side grows by what they gave. The filler absorbs its share implicitly,
/// so the column height stays the same.
#[cfg(test)]
fn resize(layout: &mut DockLayout, heights: &[(Group, f32)], i: usize, dy: f32) {
    resize_fit(layout, heights, i, dy, &[], false);
}

/// Evora build (patch LUM-18): the splitter drag above, with the groups in `folded` taken as
/// collapsed ([`fold_to_fit`]) and, with `comfort` (the Evora themes), the neighbours shrinking
/// down to [`Group::comfort_height`] instead of their minimum. Without them this is upstream's.
fn resize_fit(layout: &mut DockLayout, heights: &[(Group, f32)], i: usize, dy: f32, folded: &[Group], comfort: bool) {
    if !dy.is_finite() || dy == 0.0 {
        return;
    }
    let expanded = |k: &usize| heights.get(*k).is_some_and(|(g, _)| !layout.is_collapsed(*g) && !folded.contains(g));
    let filler = (0..heights.len()).rev().find(expanded);
    let Some(j) = (i + 1..heights.len()).find(expanded) else { return };
    if !expanded(&i) {
        return;
    }
    // Dragging down grows group `i` at the expense of the groups below; dragging up grows the
    // next expanded group at the expense of `i` and the groups above it.
    let (grow, shrink): (usize, Vec<usize>) =
        if dy > 0.0 { (i, (j..heights.len()).filter(expanded).collect()) } else { (j, (0..=i).rev().filter(expanded).collect()) };
    // The first drag pins the other groups at the heights they show, so groups still at their
    // defaults (which give way to the filler) don't shift while this one is resized.
    for (k, (o, oh)) in heights.iter().enumerate() {
        if Some(k) != filler && !layout.is_collapsed(*o) && !folded.contains(o) {
            layout.heights.entry(*o).or_insert(*oh);
        }
    }
    let mut left = dy.abs();
    for k in shrink {
        if left <= 0.0 {
            break;
        }
        let Some(&(o, oh)) = heights.get(k) else { continue };
        let floor = if comfort { o.comfort_height() } else { o.min_height() };
        let give = (oh - floor).max(0.0).min(left);
        left -= give;
        if Some(k) != filler {
            layout.heights.insert(o, oh - give);
        }
    }
    if let Some(&(o, oh)) = heights.get(grow)
        && Some(grow) != filler
    {
        layout.heights.insert(o, oh + (dy.abs() - left));
    }
}

/// What `prefs.panelLayout` holds: the live layout and open panels.
/// Evora build (patch LUM-18): and, once Evora's layout was adopted, its revision (`evoraLayout`);
/// the key is absent until then, so upstream's themes remember what they always did.
fn snapshot(app: &EvoPhotoApp) -> Value {
    let mut v = json!({"workspace": app.ui.workspace, "panels": app.ui.panels, "dockTabs": app.ui.dock_tabs, "dock": app.ui.dock, "timelineOpen": app.ui.timeline.open, "docking": app.ui.docking, "dockingHidden": app.ui.docking_hidden, "dockingCollapsed": app.ui.docking_collapsed});
    if app.ui.dock_fit.revision > 0 {
        v["evoraLayout"] = json!(app.ui.dock_fit.revision);
    }
    v
}

/// Remember the layout in the preferences once the user lets go of the mouse (Workspace ›
/// Remember Workspace Changes). Cheap: a small JSON compare per frame.
pub fn persist(app: &mut EvoPhotoApp, ctx: &egui::Context) {
    if !app.session.prefs().workspace.remember_workspace_changes || ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    let now = snapshot(app);
    if app.session.prefs().panel_layout != now {
        app.session.prefs.edit(|p| p.panel_layout = now);
    }
}

/// Restore the remembered layout at launch. Unreadable parts keep their defaults.
pub fn restore(app: &mut EvoPhotoApp) {
    if !app.session.prefs().workspace.remember_workspace_changes {
        return;
    }
    let saved = app.session.prefs().panel_layout.clone();
    apply(app, &saved);
    // Evora build (patch LUM-18): the revision of Evora's layout this one was remembered under
    // (absent or unreadable: 0). Read here only: a saved workspace applied later carries none.
    app.ui.dock_fit.revision = saved.get("evoraLayout").and_then(Value::as_u64).and_then(|r| u32::try_from(r).ok()).unwrap_or(0);
    // The floating Brush Settings window is transient, not a dock panel: remembering the
    // workspace must not leave it open on a fresh launch, especially with no document.
    // Explicit workspace switches still use `apply` and keep their stored panel visibility.
    app.ui.panels.brush_settings = false;
    if let Some(ws) = saved.get("workspace").and_then(Value::as_str) {
        app.ui.workspace = ws.to_string();
    }
}

/// Apply the `panels`, `dockTabs`, `dock` and `timelineOpen` parts of a saved layout
/// (a workspace or `panelLayout`). Missing or invalid dock parts are left alone; old
/// layouts without Timeline visibility restore it closed.
pub fn apply(app: &mut EvoPhotoApp, v: &Value) {
    app.ui.docking_collapsed = v.get("dockingCollapsed").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    app.ui.docking_reveal = None;
    app.ui.docking_generation = app.ui.docking_generation.wrapping_add(1);
    app.ui.docking_hidden = v.get("dockingHidden").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    app.ui.docking = v.get("docking").and_then(|d| serde_json::from_value(d.clone()).ok()).filter(crate::panel_docking::valid);
    if let Some(mut p) = v.get("panels").and_then(|p| serde_json::from_value::<Panels>(p.clone()).ok()) {
        // The embedding host owns these flags, including for layouts that serialize them.
        p.menu_bar = app.ui.panels.menu_bar;
        p.rail = app.ui.panels.rail;
        app.ui.panels = p;
    }
    if let Some(t) = v.get("dockTabs").and_then(|t| serde_json::from_value(t.clone()).ok()) {
        app.ui.dock_tabs = t;
    }
    if let Some(d) = v.get("dock").and_then(|d| serde_json::from_value(d.clone()).ok()) {
        app.ui.dock = d;
    }
    crate::panel_docking::sync_visibility(app);
    app.ui.timeline.open = v.get("timelineOpen").and_then(Value::as_bool).unwrap_or(false);
    if !app.ui.timeline.open {
        app.ui.timeline.playing = false;
    }
}

#[cfg(test)]
#[path = "dock_tests.rs"]
mod tests;
