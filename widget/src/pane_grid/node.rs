use crate::core::{Rectangle, Size};
use crate::pane_grid::{Axis, Pane, Split};

use std::collections::BTreeMap;

/// A layout node of a [`PaneGrid`].
///
/// [`PaneGrid`]: super::PaneGrid
#[derive(Debug, Clone)]
pub enum Node {
    /// The region of this [`Node`] is split into two.
    Split {
        /// The [`Split`] of this [`Node`].
        id: Split,

        /// The direction of the split.
        axis: Axis,

        /// The ratio of the split in [0.0, 1.0].
        ratio: f32,

        /// The left/top [`Node`] of the split.
        a: Box<Node>,

        /// The right/bottom [`Node`] of the split.
        b: Box<Node>,
    },
    /// The region of this [`Node`] is taken by a [`Pane`].
    Pane(Pane),
}

impl Node {
    /// Returns an iterator over each [`Split`] in this [`Node`].
    pub fn splits(&self) -> impl Iterator<Item = &Split> {
        let mut unvisited_nodes = vec![self];

        std::iter::from_fn(move || {
            while let Some(node) = unvisited_nodes.pop() {
                if let Node::Split { id, a, b, .. } = node {
                    unvisited_nodes.push(a);
                    unvisited_nodes.push(b);

                    return Some(id);
                }
            }

            None
        })
    }

    /// Returns the rectangular region for each [`Pane`] in the [`Node`] given
    /// the spacing between panes and the total available space.
    pub fn pane_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
    ) -> BTreeMap<Pane, Rectangle> {
        self.pane_regions_with_min_sizes(
            spacing,
            min_size,
            &BTreeMap::new(),
            bounds,
        )
    }

    /// Returns the rectangular region for each [`Pane`] with optional
    /// pane-specific minimum sizes. Panes without an override use `min_size`.
    pub fn pane_regions_with_min_sizes(
        &self,
        spacing: f32,
        min_size: f32,
        min_sizes: &BTreeMap<Pane, Size>,
        bounds: Size,
    ) -> BTreeMap<Pane, Rectangle> {
        let mut regions = BTreeMap::new();

        self.compute_regions(
            spacing,
            min_size,
            min_sizes,
            &Rectangle {
                x: 0.0,
                y: 0.0,
                width: bounds.width,
                height: bounds.height,
            },
            &mut regions,
        );

        regions
    }

    /// Returns the axis, rectangular region, and ratio for each [`Split`] in
    /// the [`Node`] given the spacing between panes and the total available
    /// space.
    pub fn split_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
    ) -> BTreeMap<Split, (Axis, Rectangle, f32)> {
        self.split_regions_with_min_sizes(
            spacing,
            min_size,
            &BTreeMap::new(),
            bounds,
        )
    }

    /// Returns split regions while respecting pane-specific minimum sizes.
    /// Panes without an override use the uniform `min_size` fallback.
    pub fn split_regions_with_min_sizes(
        &self,
        spacing: f32,
        min_size: f32,
        min_sizes: &BTreeMap<Pane, Size>,
        bounds: Size,
    ) -> BTreeMap<Split, (Axis, Rectangle, f32)> {
        let mut splits = BTreeMap::new();

        self.compute_splits(
            spacing,
            min_size,
            min_sizes,
            &Rectangle {
                x: 0.0,
                y: 0.0,
                width: bounds.width,
                height: bounds.height,
            },
            &mut splits,
        );

        splits
    }

    pub(crate) fn find(&mut self, pane: Pane) -> Option<&mut Node> {
        match self {
            Node::Split { a, b, .. } => {
                a.find(pane).or_else(move || b.find(pane))
            }
            Node::Pane(p) => {
                if *p == pane {
                    Some(self)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn split(&mut self, id: Split, axis: Axis, new_pane: Pane) {
        *self = Node::Split {
            id,
            axis,
            ratio: 0.5,
            a: Box::new(self.clone()),
            b: Box::new(Node::Pane(new_pane)),
        };
    }

    pub(crate) fn split_inverse(&mut self, id: Split, axis: Axis, pane: Pane) {
        *self = Node::Split {
            id,
            axis,
            ratio: 0.5,
            a: Box::new(Node::Pane(pane)),
            b: Box::new(self.clone()),
        };
    }

    pub(crate) fn update(&mut self, f: &impl Fn(&mut Node)) {
        if let Node::Split { a, b, .. } = self {
            a.update(f);
            b.update(f);
        }

        f(self);
    }

    pub(crate) fn resize(&mut self, split: Split, percentage: f32) -> bool {
        match self {
            Node::Split {
                id, ratio, a, b, ..
            } => {
                if *id == split {
                    *ratio = percentage;

                    true
                } else if a.resize(split, percentage) {
                    true
                } else {
                    b.resize(split, percentage)
                }
            }
            Node::Pane(_) => false,
        }
    }

    pub(crate) fn remove(&mut self, pane: Pane) -> Option<Pane> {
        match self {
            Node::Split { a, b, .. } => {
                if a.pane() == Some(pane) {
                    *self = *b.clone();
                    Some(self.first_pane())
                } else if b.pane() == Some(pane) {
                    *self = *a.clone();
                    Some(self.first_pane())
                } else {
                    a.remove(pane).or_else(|| b.remove(pane))
                }
            }
            Node::Pane(_) => None,
        }
    }

    fn pane(&self) -> Option<Pane> {
        match self {
            Node::Split { .. } => None,
            Node::Pane(pane) => Some(*pane),
        }
    }

    fn first_pane(&self) -> Pane {
        match self {
            Node::Split { a, .. } => a.first_pane(),
            Node::Pane(pane) => *pane,
        }
    }

    fn compute_regions(
        &self,
        spacing: f32,
        min_size: f32,
        min_sizes: &BTreeMap<Pane, Size>,
        current: &Rectangle,
        regions: &mut BTreeMap<Pane, Rectangle>,
    ) {
        match self {
            Node::Split {
                axis, ratio, a, b, ..
            } => {
                let minimum_a = a.minimum_size(spacing, min_size, min_sizes);
                let minimum_b = b.minimum_size(spacing, min_size, min_sizes);
                let (minimum_a, minimum_b) = match axis {
                    Axis::Horizontal => (minimum_a.height, minimum_b.height),
                    Axis::Vertical => (minimum_a.width, minimum_b.width),
                };

                let (region_a, region_b, _ratio) =
                    axis.split(current, *ratio, spacing, minimum_a, minimum_b);

                a.compute_regions(
                    spacing, min_size, min_sizes, &region_a, regions,
                );
                b.compute_regions(
                    spacing, min_size, min_sizes, &region_b, regions,
                );
            }
            Node::Pane(pane) => {
                let _ = regions.insert(*pane, *current);
            }
        }
    }

    fn compute_splits(
        &self,
        spacing: f32,
        min_size: f32,
        min_sizes: &BTreeMap<Pane, Size>,
        current: &Rectangle,
        splits: &mut BTreeMap<Split, (Axis, Rectangle, f32)>,
    ) {
        match self {
            Node::Split {
                axis,
                ratio,
                a,
                b,
                id,
            } => {
                let minimum_a = a.minimum_size(spacing, min_size, min_sizes);
                let minimum_b = b.minimum_size(spacing, min_size, min_sizes);
                let (minimum_a, minimum_b) = match axis {
                    Axis::Horizontal => (minimum_a.height, minimum_b.height),
                    Axis::Vertical => (minimum_a.width, minimum_b.width),
                };

                let (region_a, region_b, ratio) =
                    axis.split(current, *ratio, spacing, minimum_a, minimum_b);

                let _ = splits.insert(*id, (*axis, *current, ratio));

                a.compute_splits(
                    spacing, min_size, min_sizes, &region_a, splits,
                );
                b.compute_splits(
                    spacing, min_size, min_sizes, &region_b, splits,
                );
            }
            Node::Pane(_) => {}
        }
    }

    fn minimum_size(
        &self,
        spacing: f32,
        fallback: f32,
        min_sizes: &BTreeMap<Pane, Size>,
    ) -> Size {
        match self {
            Node::Pane(pane) => min_sizes
                .get(pane)
                .copied()
                .unwrap_or(Size::new(fallback, fallback)),
            Node::Split { axis, a, b, .. } => {
                let a = a.minimum_size(spacing, fallback, min_sizes);
                let b = b.minimum_size(spacing, fallback, min_sizes);
                match axis {
                    Axis::Horizontal => Size::new(
                        a.width.max(b.width),
                        a.height + b.height + spacing,
                    ),
                    Axis::Vertical => Size::new(
                        a.width + b.width + spacing,
                        a.height.max(b.height),
                    ),
                }
            }
        }
    }
}

impl std::hash::Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Node::Split {
                id,
                axis,
                ratio,
                a,
                b,
            } => {
                id.hash(state);
                axis.hash(state);
                ((ratio * 100_000.0) as u32).hash(state);
                a.hash(state);
                b.hash(state);
            }
            Node::Pane(pane) => {
                pane.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_specific_minimum_widths_are_respected() {
        let node = Node::Split {
            id: Split(0),
            axis: Axis::Vertical,
            ratio: 0.2,
            a: Box::new(Node::Pane(Pane(0))),
            b: Box::new(Node::Pane(Pane(1))),
        };
        let min_sizes = BTreeMap::from([
            (Pane(0), Size::new(250.0, 100.0)),
            (Pane(1), Size::new(350.0, 100.0)),
        ]);

        let regions = node.pane_regions_with_min_sizes(
            10.0,
            50.0,
            &min_sizes,
            Size::new(1000.0, 600.0),
        );

        assert!(regions[&Pane(0)].width >= 250.0);
        assert!(regions[&Pane(1)].width >= 350.0);
    }

    #[test]
    fn split_regions_use_the_same_pane_specific_constraints_as_layout() {
        let node = Node::Split {
            id: Split(0),
            axis: Axis::Vertical,
            ratio: 0.2,
            a: Box::new(Node::Pane(Pane(0))),
            b: Box::new(Node::Pane(Pane(1))),
        };
        let min_sizes = BTreeMap::from([
            (Pane(0), Size::new(250.0, 100.0)),
            (Pane(1), Size::new(350.0, 100.0)),
        ]);

        let split = node
            .split_regions_with_min_sizes(
                10.0,
                50.0,
                &min_sizes,
                Size::new(1000.0, 600.0),
            )
            .remove(&Split(0))
            .expect("split should be present");

        assert!(split.1.width >= 610.0);
        assert!(split.2 >= 0.25);
    }
}
