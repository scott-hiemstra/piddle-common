//! Bubble scene: model, layout simulation and camera, without any drawing.
//!
//! Positions are driven by a damped, position-based force simulation. Existing
//! bubbles are never re-placed: they only drift when neighbours grow, shrink or
//! appear, which keeps the picture stable and readable. Radius and color are
//! eased towards each new sample so changes animate smoothly between samples.
//!
//! The scene is generic over what a bubble stands for (a process in PIDdle, a
//! pod in kpiddle) through [`Item`]. Each app filters its items, decides how
//! they group, and draws the scene with its toolkit from the public fields.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::hash::Hash;
use std::sync::Arc;

/// What a bubble stands for.
pub trait Item {
    /// Identity across samples: a PID, a pod UID.
    type Key: Clone + Eq + Hash + Ord;
    /// The app's data for a group (label, kind).
    type Group: Clone;
    /// The app's per-bubble render cache (text layouts), kept across samples.
    type Cache: Default;

    fn key(&self) -> Self::Key;
    /// False when the key was reused by a different object (a recycled PID),
    /// so the old bubble fades out and a new one appears.
    fn same_instance(&self, other: &Self) -> bool;
    fn parent(&self) -> Option<Self::Key>;
    /// Target radius in world units.
    fn radius(&self) -> f64;
    /// Value the bubble's color eases towards (CPU percent of one core).
    fn cpu(&self) -> f64;
    /// Memory, summed per group.
    fn mem(&self) -> u64;
    /// Items with equal affinity start next to each other on the first sample.
    fn affinity(&self) -> &str {
        ""
    }
}

/// How urgently the scene needs redrawing after a simulation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Motion {
    /// Nothing visible is changing; stop the frame clock.
    Idle,
    /// Only gentle color easing or sub-pixel drift; a reduced frame rate is enough.
    Subtle,
    /// Bubbles are moving, growing, fading or the camera is zooming.
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrangement {
    /// One loose cluster around the center.
    Free,
    /// Clusters per group key, packed as circles, each with a shared hull.
    Grouped,
    /// Radial tree by parent.
    Tree,
}

/// A group key and the app's data for it.
pub struct GroupSpec<G> {
    pub key: Arc<str>,
    pub info: G,
}

pub struct Bubble<T: Item> {
    pub item: T,
    pub cache: T::Cache,
    pub x: f64,
    pub y: f64,
    vx: f64,
    vy: f64,
    pub r: f64,
    target_r: f64,
    /// Displayed (eased) CPU percentage, drives color.
    pub cpu: f64,
    pub alpha: f64,
    pub dying: bool,
    /// Matches the current search (see [`Scene::set_matches`]).
    pub matched: bool,
    generation: u64,
    spec: Option<GroupSpec<T::Group>>,
    pub group: usize,
    pub parent: Option<usize>,
    /// Number of visible children (Tree arrangement only).
    pub kids: usize,
    /// Radial-tree target position (Tree arrangement only).
    tx: f64,
    ty: f64,
}

pub struct Group<G> {
    pub key: Arc<str>,
    pub info: G,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    r: f64,
    /// Drawn outline: centered on the members' centroid.
    pub cx: f64,
    pub cy: f64,
    pub hull: f64,
    pub members: usize,
    /// Sum of the members' displayed CPU.
    pub cpu: f64,
    pub mem: u64,
}

pub struct Scene<T: Item> {
    pub bubbles: Vec<Bubble<T>>,
    index: HashMap<T::Key, usize>,
    pub groups: Vec<Group<T::Group>>,
    arrangement: Arrangement,
    pub hover: Option<T::Key>,
    pub mouse: (f64, f64),
    pub size: (f64, f64),
    cam: (f64, f64, f64),
    cam_target: (f64, f64, f64),
    cam_init: bool,
    generation: u64,
    order: Vec<usize>,
    prev: Vec<(f64, f64)>,
    has_query: bool,
    /// Live bubbles matching the current search.
    pub matches: usize,
    /// In the Grouped arrangement, spawn new bubbles next to their parent
    /// bubble rather than at their group (PIDdle's by-parent layout).
    pub spawn_near_parent: bool,
}

impl<T: Item> Default for Scene<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Item> Scene<T> {
    pub fn new() -> Self {
        Self {
            bubbles: Vec::new(),
            index: HashMap::new(),
            groups: Vec::new(),
            arrangement: Arrangement::Free,
            hover: None,
            mouse: (0.0, 0.0),
            size: (800.0, 600.0),
            cam: (0.0, 0.0, 1.0),
            cam_target: (0.0, 0.0, 1.0),
            cam_init: false,
            generation: 0,
            order: Vec::new(),
            prev: Vec::new(),
            has_query: false,
            matches: 0,
            spawn_near_parent: false,
        }
    }

    /// Current world→screen scale, used by legends to draw true-size references.
    pub fn scale(&self) -> f64 {
        self.cam.2
    }

    pub fn arrangement(&self) -> Arrangement {
        self.arrangement
    }

    /// A live (not fading out) bubble.
    pub fn bubble(&self, key: &T::Key) -> Option<&Bubble<T>> {
        self.index.get(key).map(|&i| &self.bubbles[i])
    }

    /// Reconcile bubbles with the latest (already filtered) items.
    /// `group_of` gives each item's group in the current arrangement.
    pub fn sync(&mut self, items: Vec<T>, group_of: impl Fn(&T) -> GroupSpec<T::Group>) {
        self.generation += 1;
        let generation = self.generation;
        let startup = self.bubbles.is_empty();
        let mut fresh = Vec::new();

        for it in items {
            match self.index.get(&it.key()) {
                Some(&i) if self.bubbles[i].item.same_instance(&it) => {
                    let b = &mut self.bubbles[i];
                    // Hysteresis: ignore small size wobble so the layout can rest.
                    let r = it.radius();
                    if (r - b.target_r).abs() > b.target_r * 0.02 {
                        b.target_r = r;
                    }
                    b.item = it;
                    b.generation = generation;
                    b.dying = false;
                }
                other => {
                    if let Some(&i) = other {
                        self.bubbles[i].dying = true; // key reused
                    }
                    fresh.push(it);
                }
            }
        }

        for b in &mut self.bubbles {
            if b.generation != generation {
                b.dying = true;
            }
        }
        // Place new bubbles after parents are known so children spawn near them.
        let mut fresh_idx = Vec::with_capacity(fresh.len());
        for it in fresh {
            self.bubbles.push(Bubble {
                cpu: it.cpu(),
                target_r: it.radius(),
                item: it,
                cache: T::Cache::default(),
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                r: 0.0,
                alpha: 0.0,
                dying: false,
                matched: false,
                generation,
                spec: None,
                group: 0,
                parent: None,
                kids: 0,
                tx: 0.0,
                ty: 0.0,
            });
            fresh_idx.push(self.bubbles.len() - 1);
        }
        for b in &mut self.bubbles {
            b.spec = Some(group_of(&b.item));
        }
        self.reindex();
        self.rebuild_groups();
        self.layout_tree();
        if startup {
            self.initial_placement();
        } else {
            for i in fresh_idx {
                self.place(i);
            }
        }
    }

    /// Switch arrangement and regroup every bubble with `group_of`.
    pub fn set_arrangement(&mut self, a: Arrangement, group_of: impl Fn(&T) -> GroupSpec<T::Group>) {
        self.arrangement = a;
        for b in &mut self.bubbles {
            b.spec = Some(group_of(&b.item));
        }
        self.rebuild_groups();
        self.layout_tree();
        for b in &mut self.bubbles {
            b.vx = 0.0;
            b.vy = 0.0;
        }
    }

    /// Mark bubbles matching a search. With `active` false, nothing matches
    /// and nothing is dimmed.
    pub fn set_matches(&mut self, active: bool, matches: impl Fn(&T) -> bool) {
        self.has_query = active;
        let mut n = 0;
        for b in &mut self.bubbles {
            b.matched = active && matches(&b.item);
            if b.matched && !b.dying {
                n += 1;
            }
        }
        self.matches = n;
    }

    pub fn has_query(&self) -> bool {
        self.has_query
    }

    fn reindex(&mut self) {
        self.index.clear();
        for (i, b) in self.bubbles.iter().enumerate() {
            if !b.dying {
                self.index.insert(b.item.key(), i);
            }
        }
        for i in 0..self.bubbles.len() {
            let parent = self.bubbles[i].item.parent();
            self.bubbles[i].parent = parent.and_then(|p| self.index.get(&p).copied()).filter(|&p| p != i);
        }
    }

    fn rebuild_groups(&mut self) {
        let mut old: HashMap<Arc<str>, Group<T::Group>> = self.groups.drain(..).map(|g| (g.key.clone(), g)).collect();
        let mut map: HashMap<Arc<str>, usize> = HashMap::new();
        let mut groups: Vec<Group<T::Group>> = Vec::new();
        let mut fresh_groups = Vec::new();
        for b in &mut self.bubbles {
            let Some(spec) = &b.spec else { continue };
            let gi = match map.get(&spec.key) {
                Some(&gi) => gi,
                None => {
                    let g = match old.remove(&spec.key) {
                        Some(mut g) => {
                            g.info = spec.info.clone();
                            g
                        }
                        None => {
                            fresh_groups.push(groups.len());
                            Group {
                                key: spec.key.clone(),
                                info: spec.info.clone(),
                                x: 0.0,
                                y: 0.0,
                                vx: 0.0,
                                vy: 0.0,
                                r: 0.0,
                                cx: 0.0,
                                cy: 0.0,
                                hull: 0.0,
                                members: 0,
                                cpu: 0.0,
                                mem: 0,
                            }
                        }
                    };
                    map.insert(spec.key.clone(), groups.len());
                    groups.push(g);
                    groups.len() - 1
                }
            };
            b.group = gi;
        }
        // New groups start at the centroid of their (already placed) members.
        for gi in fresh_groups {
            let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
            for b in self.bubbles.iter().filter(|b| b.group == gi && b.r > 0.0) {
                sx += b.x;
                sy += b.y;
                n += 1.0;
            }
            if n > 0.0 {
                groups[gi].x = sx / n;
                groups[gi].y = sy / n;
            } else {
                let (x, y) = spiral(gi, 60.0);
                groups[gi].x = x;
                groups[gi].y = y;
            }
        }
        self.groups = groups;
    }
    /// Lay out the very first sample on a sunflower spiral, sorted so that
    /// related items start next to each other.
    fn initial_placement(&mut self) {
        let mut order: Vec<usize> = (0..self.bubbles.len()).collect();
        order.sort_by(|&a, &b| {
            let (pa, pb) = (&self.bubbles[a].item, &self.bubbles[b].item);
            pa.affinity().cmp(pb.affinity()).then(pa.key().cmp(&pb.key()))
        });
        let spacing = 2.0 * self.bubbles.iter().map(|b| b.target_r).sum::<f64>() / self.bubbles.len().max(1) as f64;
        for (k, &i) in order.iter().enumerate() {
            let (x, y) = spiral(k, spacing * 0.75);
            let b = &mut self.bubbles[i];
            b.x = x * 1.4;
            b.y = y;
            b.r = b.target_r * 0.5;
        }
        for g in 0..self.groups.len() {
            let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
            for b in self.bubbles.iter().filter(|b| b.group == g) {
                sx += b.x;
                sy += b.y;
                n += 1.0;
            }
            self.groups[g].x = sx / n.max(1.0);
            self.groups[g].y = sy / n.max(1.0);
        }
    }

    /// Find a free spot for a new bubble near where it logically belongs.
    fn place(&mut self, i: usize) {
        let b = &self.bubbles[i];
        let near_parent = self.arrangement == Arrangement::Tree
            || (self.arrangement == Arrangement::Grouped && self.spawn_near_parent);
        let (ax, ay) = match (self.arrangement, b.parent) {
            (_, Some(p)) if near_parent => (self.bubbles[p].x, self.bubbles[p].y),
            (Arrangement::Grouped, _) => (self.groups[b.group].x, self.groups[b.group].y),
            (_, Some(p)) => (self.bubbles[p].x, self.bubbles[p].y),
            _ => (0.0, 0.0),
        };
        let r = b.target_r;
        let step = (r * 0.8).max(6.0);
        let mut best = (ax, ay);
        'search: for k in 0..120 {
            let ang = k as f64 * 2.399_963;
            let d = (k as f64).sqrt() * step;
            let (x, y) = (ax + ang.cos() * d, ay + ang.sin() * d);
            for (j, o) in self.bubbles.iter().enumerate() {
                if j == i || o.dying || o.r <= 0.0 {
                    continue;
                }
                let rr = o.r + r + 3.0;
                if (o.x - x).powi(2) + (o.y - y).powi(2) < rr * rr {
                    continue 'search;
                }
            }
            best = (x, y);
            break;
        }
        let b = &mut self.bubbles[i];
        b.x = best.0;
        b.y = best.1;
    }

    // ---------------------------------------------------------------- simulation

    /// Advance animation by `dt` seconds and report how much is still changing.
    pub fn step(&mut self, dt: f64) -> Motion {
        let dt = dt.clamp(1.0 / 240.0, 0.1);
        let k = 1.0 - (-dt / 0.15).exp();
        let scale = self.cam.2;
        let mut subtle = false;
        let mut active = false;

        for b in &mut self.bubbles {
            let ta = if b.dying { 0.0 } else { 1.0 };
            let tr = if b.dying { b.target_r * 0.6 } else { b.target_r };
            let target_cpu = b.item.cpu();
            let dr = tr - b.r;
            let dc = target_cpu - b.cpu;
            let da = ta - b.alpha;
            b.r += dr * k;
            b.cpu += dc * k;
            if dr.abs() * scale <= 0.2 {
                b.r = tr;
            }
            if dc.abs() <= 1.0 {
                b.cpu = target_cpu;
            }
            b.alpha += da * (k * 1.4).min(1.0);
            // Stop animating once remaining changes are imperceptible.
            if dr.abs() * scale > 0.2 || da.abs() > 0.01 {
                active = true;
            } else if dc.abs() > 1.0 {
                subtle = true;
            }
        }

        if self.bubbles.iter().any(|b| b.dying && b.alpha < 0.02) {
            self.bubbles.retain(|b| !(b.dying && b.alpha < 0.02));
            self.reindex();
            self.rebuild_groups();
            self.layout_tree();
            if let Some(h) = &self.hover
                && !self.index.contains_key(h)
            {
                self.hover = None;
            }
        }

        let steps = ((dt * 60.0).round() as usize).clamp(1, 3);
        let mut moving = 0.0f64;
        for _ in 0..steps {
            moving = moving.max(self.physics());
        }
        // Measured in screen pixels so the scene rests regardless of zoom.
        let moved = moving * self.cam.2;
        active |= moved > 0.6;
        subtle |= moved > 0.15;
        active |= self.update_camera(dt);
        if active {
            Motion::Active
        } else if subtle {
            Motion::Subtle
        } else {
            Motion::Idle
        }
    }

    fn physics(&mut self) -> f64 {
        let (w, h) = self.size;
        let sa = (w / h.max(1.0)).clamp(0.5, 3.0).sqrt();
        let n = self.bubbles.len();
        let mut acc = vec![(0.0f64, 0.0f64); n];

        match self.arrangement {
            Arrangement::Free => {
                let g = 0.004;
                for (i, b) in self.bubbles.iter().enumerate() {
                    acc[i] = (-b.x * g / sa, -b.y * g * sa);
                }
            }
            Arrangement::Grouped => {
                self.group_physics(sa);
                for (i, b) in self.bubbles.iter().enumerate() {
                    let g = &self.groups[b.group];
                    acc[i] = ((g.x - b.x) * 0.03, (g.y - b.y) * 0.03);
                }
            }
            Arrangement::Tree => {
                for (i, b) in self.bubbles.iter().enumerate() {
                    acc[i] = ((b.tx - b.x) * 0.04, (b.ty - b.y) * 0.04);
                }
            }
        }

        // Position-based dynamics: integrate, resolve collisions, then derive
        // velocity from the actual displacement so pushes don't build up energy.
        let mut prev = std::mem::take(&mut self.prev);
        prev.clear();
        prev.extend(self.bubbles.iter().map(|b| (b.x, b.y)));
        for (b, a) in self.bubbles.iter_mut().zip(&acc) {
            b.vx += a.0;
            b.vy += a.1;
            let v = (b.vx * b.vx + b.vy * b.vy).sqrt();
            if v > 25.0 {
                b.vx *= 25.0 / v;
                b.vy *= 25.0 / v;
            }
            b.x += b.vx;
            b.y += b.vy;
        }
        self.collide();
        let mut max_d = 0.0f64;
        for (b, &(px, py)) in self.bubbles.iter_mut().zip(&prev) {
            let (dx, dy) = (b.x - px, b.y - py);
            b.vx = dx * 0.72;
            b.vy = dy * 0.72;
            if !b.dying {
                max_d = max_d.max(dx.abs().max(dy.abs()));
            }
        }
        self.prev = prev;
        max_d
    }

    fn group_physics(&mut self, sa: f64) {
        for g in &mut self.groups {
            g.members = 0;
            g.r = 0.0;
            g.cpu = 0.0;
            g.mem = 0;
            g.cx = 0.0;
            g.cy = 0.0;
            g.hull = 0.0;
        }
        for b in self.bubbles.iter().filter(|b| !b.dying) {
            let g = &mut self.groups[b.group];
            g.members += 1;
            g.r += (b.r + 3.0).powi(2);
            g.cpu += b.cpu;
            g.mem += b.item.mem();
            g.cx += b.x;
            g.cy += b.y;
        }
        for g in &mut self.groups {
            if g.members > 0 {
                g.cx /= g.members as f64;
                g.cy /= g.members as f64;
            }
        }
        for b in self.bubbles.iter().filter(|b| !b.dying) {
            let g = &mut self.groups[b.group];
            let d = ((b.x - g.cx).powi(2) + (b.y - g.cy).powi(2)).sqrt() + b.r;
            g.hull = g.hull.max(d);
        }

        // Groups are themselves circles packed with position-based dynamics.
        let prev: Vec<(f64, f64)> = self.groups.iter().map(|g| (g.x, g.y)).collect();
        for g in &mut self.groups {
            g.hull += 7.0;
            g.r = (g.r.sqrt() * 1.15 + 8.0).max(g.hull);
            g.vx -= g.x * 0.004 / sa;
            g.vy -= g.y * 0.004 * sa;
            g.x += g.vx;
            g.y += g.vy;
        }
        let m = self.groups.len();
        for _ in 0..2 {
            for i in 0..m {
                if self.groups[i].members == 0 {
                    continue;
                }
                for j in i + 1..m {
                    if self.groups[j].members == 0 {
                        continue;
                    }
                    let (a, b) = (&self.groups[i], &self.groups[j]);
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let min = a.r + b.r + 16.0;
                    if dx.abs() >= min || dy.abs() >= min {
                        continue;
                    }
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < min {
                        let (nx, ny) = if d > 1e-6 { (dx / d, dy / d) } else { unit(i * 31 + j) };
                        let (ma, mb) = (a.r * a.r, b.r * b.r);
                        let push = (min - d) * 0.8;
                        let wa = mb / (ma + mb);
                        let wb = ma / (ma + mb);
                        self.groups[i].x -= nx * push * wa;
                        self.groups[i].y -= ny * push * wa;
                        self.groups[j].x += nx * push * wb;
                        self.groups[j].y += ny * push * wb;
                    }
                }
            }
        }
        for (g, (px, py)) in self.groups.iter_mut().zip(prev) {
            g.vx = (g.x - px) * 0.72;
            g.vy = (g.y - py) * 0.72;
        }
    }

    /// Radial tree: the root sits in the middle, each generation on its own
    /// ring, and every subtree owns an angular wedge proportional to its size.
    /// Children are ordered by key so the layout stays put between samples.
    fn layout_tree(&mut self) {
        if self.arrangement != Arrangement::Tree {
            return;
        }
        const GAP: f64 = 6.0;
        let n = self.bubbles.len();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut roots = Vec::new();
        for i in 0..n {
            if self.bubbles[i].dying {
                continue;
            }
            match self.bubbles[i].parent.filter(|&p| !self.bubbles[p].dying) {
                Some(p) => children[p].push(i),
                None => roots.push(i),
            }
        }
        let key = |i: &usize| self.bubbles[*i].item.key();
        for c in &mut children {
            c.sort_by_key(key);
        }
        roots.sort_by_key(key);
        for (b, c) in self.bubbles.iter_mut().zip(&children) {
            b.kids = c.len();
        }

        // Post-order weights and depths (iterative: chains can be deep).
        let single_root = roots.len() == 1;
        let mut depth = vec![0usize; n];
        let mut order = Vec::with_capacity(n);
        let mut stack: Vec<(usize, usize)> = roots.iter().map(|&r| (r, if single_root { 0 } else { 1 })).collect();
        while let Some((i, d)) = stack.pop() {
            depth[i] = d;
            order.push(i);
            stack.extend(children[i].iter().map(|&c| (c, d + 1)));
        }
        let mut weight = vec![0.0f64; n];
        for &i in order.iter().rev() {
            let own = self.bubbles[i].target_r * 2.0 + GAP;
            let kids: f64 = children[i].iter().map(|&c| weight[c]).sum();
            weight[i] = own.max(kids);
        }

        // Ring radii: far enough from the previous ring, and long enough to
        // hold every bubble of that generation side by side.
        let max_depth = order.iter().map(|&i| depth[i]).max().unwrap_or(0);
        let mut ring_need = vec![0.0f64; max_depth + 1];
        let mut ring_maxr = vec![0.0f64; max_depth + 1];
        for &i in &order {
            let r = self.bubbles[i].target_r;
            ring_need[depth[i]] += r * 2.0 + GAP;
            ring_maxr[depth[i]] = ring_maxr[depth[i]].max(r);
        }
        let mut ring = vec![0.0f64; max_depth + 1];
        for d in 1..=max_depth {
            let base = if d == 1 && !single_root { 0.0 } else { ring[d - 1] + ring_maxr[d - 1] };
            ring[d] = (base + ring_maxr[d] + 34.0).max(ring_need[d] / TAU * 1.1);
        }

        // Assign wedges top-down.
        let total: f64 = roots.iter().map(|&r| weight[r]).sum::<f64>().max(1.0);
        let mut stack: Vec<(usize, f64, f64)> = Vec::new();
        let mut a = -PI / 2.0;
        for &r in &roots {
            let span = TAU * weight[r] / total;
            stack.push((r, a, span));
            a += span;
        }
        while let Some((i, a0, span)) = stack.pop() {
            let ang = a0 + span / 2.0;
            let rad = ring[depth[i]];
            self.bubbles[i].tx = rad * ang.cos();
            self.bubbles[i].ty = rad * ang.sin();
            let kids: f64 = children[i].iter().map(|&c| weight[c]).sum::<f64>().max(1e-6);
            let mut a = a0;
            for &c in &children[i] {
                let s = span * weight[c] / kids;
                stack.push((c, a, s));
                a += s;
            }
        }
    }

    /// Sort-and-sweep collision resolution (position based, no added velocity).
    fn collide(&mut self) -> f64 {
        let mut max_push = 0.0f64;
        let grouped = self.arrangement == Arrangement::Grouped;
        let max_pad = 3.0;
        let mut order = std::mem::take(&mut self.order);
        order.clear();
        order.extend((0..self.bubbles.len()).filter(|&i| !self.bubbles[i].dying));
        for _ in 0..4 {
            let bs = &mut self.bubbles;
            order.sort_unstable_by(|&a, &b| (bs[a].x - bs[a].r).total_cmp(&(bs[b].x - bs[b].r)));
            for ii in 0..order.len() {
                let i = order[ii];
                for &j in &order[ii + 1..] {
                    let (xi, yi, ri, gi) = (bs[i].x, bs[i].y, bs[i].r, bs[i].group);
                    let (xj, yj, rj) = (bs[j].x, bs[j].y, bs[j].r);
                    if xj - rj > xi + ri + max_pad {
                        break;
                    }
                    // Across groups, packing is handled at the group level so
                    // members can travel through other clusters to their own.
                    if grouped && bs[j].group != gi {
                        continue;
                    }
                    let pad = 3.0;
                    let min = ri + rj + pad;
                    let (dx, dy) = (xj - xi, yj - yi);
                    if dy.abs() >= min {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy;
                    if d2 >= min * min {
                        continue;
                    }
                    let d = d2.sqrt();
                    let (nx, ny) = if d > 1e-6 { (dx / d, dy / d) } else { unit(i * 7919 + j) };
                    let (mi, mj) = (ri * ri + 1.0, rj * rj + 1.0);
                    let push = (min - d) * 0.8;
                    max_push = max_push.max(push);
                    let wi = mj / (mi + mj);
                    let wj = mi / (mi + mj);
                    bs[i].x -= nx * push * wi;
                    bs[i].y -= ny * push * wi;
                    bs[j].x += nx * push * wj;
                    bs[j].y += ny * push * wj;
                }
            }
        }
        self.order = order;
        max_push
    }

    fn update_camera(&mut self, dt: f64) -> bool {
        let (w, h) = self.size;
        let mut bb = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for b in self.bubbles.iter().filter(|b| !b.dying) {
            bb.0 = bb.0.min(b.x - b.target_r);
            bb.1 = bb.1.min(b.y - b.target_r);
            bb.2 = bb.2.max(b.x + b.target_r);
            bb.3 = bb.3.max(b.y + b.target_r);
        }
        if bb.0 > bb.2 {
            return false;
        }
        let margin = 36.0;
        let bw = (bb.2 - bb.0).max(50.0);
        let bh = (bb.3 - bb.1).max(50.0);
        let s = ((w - 2.0 * margin) / bw).min((h - 2.0 * margin) / bh).clamp(0.05, 2.5);
        let t = ((bb.0 + bb.2) / 2.0, (bb.1 + bb.3) / 2.0, s);
        // Dead band: ignore tiny bbox wobble so the view can come to rest.
        let ct = self.cam_target;
        if !self.cam_init
            || (t.2 / ct.2 - 1.0).abs() > 0.03
            || (t.0 - ct.0).abs() * ct.2 > 12.0
            || (t.1 - ct.1).abs() * ct.2 > 12.0
            || t.2 < ct.2
        {
            self.cam_target = t;
        }
        if !self.cam_init {
            self.cam = self.cam_target;
            self.cam_init = true;
            return true;
        }
        let k = 1.0 - (-dt / 0.45).exp();
        let c = &mut self.cam;
        let tg = self.cam_target;
        c.0 += (tg.0 - c.0) * k;
        c.1 += (tg.1 - c.1) * k;
        c.2 += (tg.2 - c.2) * k;
        (tg.2 / c.2 - 1.0).abs() > 0.001 || (tg.0 - c.0).abs() * c.2 > 0.3 || (tg.1 - c.1).abs() * c.2 > 0.3
    }

    /// Screen position of a live bubble's center.
    pub fn screen_pos(&self, key: &T::Key) -> Option<(f64, f64)> {
        self.bubble(key).map(|b| self.to_screen(b.x, b.y))
    }

    pub fn to_screen(&self, x: f64, y: f64) -> (f64, f64) {
        let (w, h) = self.size;
        ((x - self.cam.0) * self.cam.2 + w / 2.0, (y - self.cam.1) * self.cam.2 + h / 2.0)
    }

    /// The live bubble under a screen position.
    pub fn pick(&self, sx: f64, sy: f64) -> Option<T::Key> {
        let (w, h) = self.size;
        let s = self.cam.2;
        let (x, y) = ((sx - w / 2.0) / s + self.cam.0, (sy - h / 2.0) / s + self.cam.1);
        let slop = 2.0 / s;
        self.bubbles
            .iter()
            .filter(|b| !b.dying)
            .find(|b| (b.x - x).powi(2) + (b.y - y).powi(2) <= (b.r + slop).powi(2))
            .map(|b| b.item.key())
    }
}

fn spiral(k: usize, spacing: f64) -> (f64, f64) {
    let ang = k as f64 * 2.399_963;
    let d = (k as f64).sqrt() * spacing;
    (ang.cos() * d, ang.sin() * d)
}

fn unit(seed: usize) -> (f64, f64) {
    let a = (seed as f64 * 2.399_963) % TAU;
    (a.cos(), a.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct It {
        id: u32,
        instance: u32,
        parent: Option<u32>,
        r: f64,
        cpu: f64,
        group: &'static str,
    }

    impl Item for It {
        type Key = u32;
        type Group = String;
        type Cache = ();
        fn key(&self) -> u32 {
            self.id
        }
        fn same_instance(&self, o: &Self) -> bool {
            self.instance == o.instance
        }
        fn parent(&self) -> Option<u32> {
            self.parent
        }
        fn radius(&self) -> f64 {
            self.r
        }
        fn cpu(&self) -> f64 {
            self.cpu
        }
        fn mem(&self) -> u64 {
            self.r as u64
        }
    }

    fn items(n: u32) -> Vec<It> {
        (0..n)
            .map(|id| It {
                id,
                instance: 0,
                parent: (id > 0).then_some(id / 3),
                r: 5.0 + (id % 7) as f64 * 4.0,
                cpu: (id * 13 % 100) as f64,
                group: ["a", "b", "c"][id as usize % 3],
            })
            .collect()
    }

    fn by_group(it: &It) -> GroupSpec<String> {
        GroupSpec { key: it.group.into(), info: format!("group {}", it.group) }
    }

    fn settle(s: &mut Scene<It>) -> usize {
        let mut frames = 0;
        while s.step(1.0 / 60.0) != Motion::Idle && frames < 3000 {
            frames += 1;
        }
        frames
    }

    fn overlaps(s: &Scene<It>, same_group_only: bool) -> usize {
        let live: Vec<_> = s.bubbles.iter().filter(|b| !b.dying).collect();
        let mut n = 0;
        for (i, a) in live.iter().enumerate() {
            for b in &live[i + 1..] {
                if same_group_only && a.group != b.group {
                    continue;
                }
                let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
                // Bubbles aim for a 3-unit gap; at rest the pull to the
                // center can squeeze that slightly, which isn't visible.
                if d < a.r + b.r - 2.0 {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn every_arrangement_settles_without_overlap() {
        for a in [Arrangement::Free, Arrangement::Grouped, Arrangement::Tree] {
            let mut s = Scene::new();
            s.set_arrangement(a, by_group);
            s.sync(items(60), by_group);
            let frames = settle(&mut s);
            assert!(frames < 3000, "{a:?} never came to rest");
            assert_eq!(overlaps(&s, false), 0, "{a:?} left bubbles overlapping");
            assert!(s.bubbles.iter().all(|b| b.r == b.target_r && b.cpu == b.item.cpu));
        }
    }

    #[test]
    fn groups_collect_their_members() {
        let mut s = Scene::new();
        s.set_arrangement(Arrangement::Grouped, by_group);
        s.sync(items(9), by_group);
        settle(&mut s);
        assert_eq!(s.groups.len(), 3);
        for g in &s.groups {
            assert_eq!(g.members, 3);
            assert_eq!(g.info, format!("group {}", g.key));
            // The hull contains every member.
            for b in s.bubbles.iter().filter(|b| s.groups[b.group].key == g.key) {
                let d = ((b.x - g.cx).powi(2) + (b.y - g.cy).powi(2)).sqrt() + b.r;
                assert!(d <= g.hull + 1e-6);
            }
        }
    }

    #[test]
    fn removed_and_reused_keys_fade_out() {
        let mut s = Scene::new();
        s.sync(items(5), by_group);
        settle(&mut s);
        // 4 is gone; 3 is a different object under the same key.
        let mut next = items(4);
        next[3].instance = 1;
        s.sync(next, by_group);
        assert_eq!(s.bubbles.iter().filter(|b| b.dying).count(), 2);
        assert_eq!(s.bubble(&3).map(|b| b.item.instance), Some(1));
        assert!(s.bubble(&4).is_none());
        settle(&mut s);
        assert_eq!(s.bubbles.len(), 4);
        assert!(s.bubbles.iter().all(|b| !b.dying));
    }

    #[test]
    fn hover_clears_when_its_bubble_is_gone() {
        let mut s = Scene::new();
        s.sync(items(3), by_group);
        s.hover = Some(2);
        s.sync(items(2), by_group);
        settle(&mut s);
        assert_eq!(s.hover, None);
    }

    #[test]
    fn matches_count_live_bubbles() {
        let mut s = Scene::new();
        s.sync(items(6), by_group);
        s.set_matches(true, |it| it.group == "a");
        assert_eq!(s.matches, 2);
        s.set_matches(false, |_| true);
        assert_eq!(s.matches, 0);
        assert!(!s.has_query());
    }

    #[test]
    fn pick_finds_the_bubble_under_the_cursor() {
        let mut s = Scene::new();
        s.sync(items(10), by_group);
        settle(&mut s);
        for b in &s.bubbles {
            let (x, y) = s.to_screen(b.x, b.y);
            assert_eq!(s.pick(x, y), Some(b.item.id));
        }
    }
}
