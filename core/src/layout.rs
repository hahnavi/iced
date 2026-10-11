//! Position your widgets properly.
mod limits;
mod node;

pub mod flex;

pub use limits::Limits;
pub use node::Node;

use crate::widget;
use crate::{Length, Padding, Point, Rectangle, Size, Vector};

/// The bounds of a widget and its children, using absolute coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    position: Point,
    size: Size,
    parent: Option<Rectangle>,
    virtual_offset: Vector,
}

impl Layout {
    /// Creates a new [`Layout`] for the given [`Size`] at the origin.
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            position: Point::ORIGIN,
            size: size.into(),
            parent: None,
            virtual_offset: Vector::ZERO,
        }
    }

    /// Creates a new [`Layout`] for the given [`Node`] with the provided offset from the origin.
    pub fn with_offset(offset: Vector, node: &Node) -> Self {
        let bounds = node.bounds();

        Self {
            position: Point::new(bounds.x, bounds.y) + offset,
            size: bounds.size(),
            parent: None,
            virtual_offset: Vector::ZERO,
        }
    }

    /// Returns a new layout with the virtual offset
    pub fn with_virtual_offset(mut self, virtual_offset: Vector) -> Self {
        self.virtual_offset = virtual_offset;
        self
    }

    /// Returns the virtual offset propagated through the widget tree.
    pub fn virtual_offset(&self) -> Vector {
        self.virtual_offset
    }

    /// Returns the position of the [`Layout`].
    pub fn position(&self) -> Point {
        self.position
    }

    /// Moves the [`Layout`] to the given position.
    pub fn move_to(self, position: impl Into<Point>) -> Self {
        let position = position.into();
        Self {
            position,
            size: self.size,
            parent: self.parent,
            virtual_offset: self.virtual_offset,
        }
    }

    /// Returns the bounds of the [`Layout`].
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.position, self.size)
    }

    /// Returns the size of the [`Layout`].
    pub fn size(&self) -> Size {
        self.size
    }

    /// Returns the bounds of the parent of this [`Layout`], if any.
    pub fn parent(&self) -> Option<Rectangle> {
        self.parent
    }

    /// Returns an iterator over the children of this [`Layout`].
    pub fn iter(
        self,
        children: &[widget::Tree],
    ) -> impl DoubleEndedIterator<Item = (Self, &widget::Tree)> + ExactSizeIterator {
        children.iter().map(move |child| (self.child(child), child))
    }

    /// Returns a mutable iterator over the children of this [`Layout`].
    pub fn iter_mut(
        self,
        children: &mut [widget::Tree],
    ) -> impl DoubleEndedIterator<Item = (Self, &mut widget::Tree)> + ExactSizeIterator {
        children
            .iter_mut()
            .map(move |child| (self.child(child), child))
    }

    fn child(&self, child: &widget::Tree) -> Self {
        Self {
            position: self.position + child.translation,
            size: child.size,
            parent: Some(self.bounds()),
            virtual_offset: self.virtual_offset,
        }
    }
}

impl From<&Node> for Size {
    fn from(node: &Node) -> Size {
        node.size()
    }
}

/// Lays out two children side by side in the provided [`widget::Tree`].
pub fn next_to_each_other(tree: &mut widget::Tree, left: Size, right: Size, spacing: f32) {
    let (left_y, right_y) = if left.height > right.height {
        (0.0, (left.height - right.height) / 2.0)
    } else {
        ((right.height - left.height) / 2.0, 0.0)
    };

    tree.size = Size::new(
        left.width + spacing + right.width,
        left.height.max(right.height),
    );

    tree.children[0].size = left;
    tree.children[0].translation = Vector::new(0.0, left_y);

    tree.children[1].size = right;
    tree.children[1].translation = Vector::new(left.width + spacing, right_y);
}

/// Computes the resulting [`Size`] that fits the [`Limits`] given
/// some width and height requirements and no intrinsic size.
pub fn atomic(limits: &Limits, width: impl Into<Length>, height: impl Into<Length>) -> Size {
    let width = width.into();
    let height = height.into();

    limits
        .width(width)
        .height(height)
        .resolve(width, height, Size::ZERO)
}

/// Computes the resulting [`Size`] that fits the [`Limits`] given
/// some width and height requirements and a closure that produces
/// the intrinsic [`Size`] inside the given [`Limits`].
pub fn sized(
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    f: impl FnOnce(&Limits) -> Size,
) -> Size {
    let width = width.into();
    let height = height.into();

    let limits = limits.width(width).height(height);
    let intrinsic_size = f(&limits);

    limits.resolve(width, height, intrinsic_size)
}

/// Computes the resulting [`Size`] that fits the [`Limits`] given
/// some width and height requirements and a closure that produces
/// the content inside the given [`Limits`].
pub fn contained(
    tree: &mut widget::Tree,
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    f: impl FnOnce(&mut widget::Tree, &Limits) -> Size,
) {
    let width = width.into();
    let height = height.into();
    let limits = limits.width(width).height(height);

    let child = &mut tree.children[0];
    let content = f(child, &limits);

    tree.size = limits.resolve(width, height, content);
    child.translation = Vector::ZERO;
}

/// Computes the [`Size`] that fits the [`Limits`] given some width, height, and
/// [`Padding`] requirements and a closure that produces the content
/// inside the given [`Limits`].
pub fn padded(
    tree: &mut widget::Tree,
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    padding: impl Into<Padding>,
    layout: impl FnOnce(&mut widget::Tree, &Limits) -> Size,
) {
    positioned(tree, limits, width, height, padding, layout, |_, _| {
        Vector::ZERO
    });
}

/// Computes a [`padded`] [`Size`] with a positioning step.
pub fn positioned(
    tree: &mut widget::Tree,
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    padding: impl Into<Padding>,
    layout: impl FnOnce(&mut widget::Tree, &Limits) -> Size,
    translate: impl FnOnce(Size, Size) -> Vector,
) {
    let width = width.into();
    let height = height.into();
    let padding = padding.into();
    let limits = limits.width(width).height(height);

    let child = &mut tree.children[0];
    let content = layout(child, &limits.shrink(padding));
    let padding = padding.fit(content, limits.bounds());

    let container = limits.shrink(padding).resolve(width, height, content);

    tree.size = container.expand(padding);
    child.translation = Vector::new(padding.left, padding.top) + translate(content, container);
}
