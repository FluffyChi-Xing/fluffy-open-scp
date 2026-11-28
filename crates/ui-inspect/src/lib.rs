//! Host-independent UI hit testing and persistent element paths.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    fn size(self) -> Option<(f64, f64)> {
        let width = self.right - self.left;
        let height = self.bottom - self.top;
        (width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0)
            .then_some((width, height))
    }

    /// Use half-open bounds so adjacent elements do not both own a shared edge.
    pub fn contains(self, point: Point) -> bool {
        self.size().is_some()
            && point.x >= self.left
            && point.x < self.right
            && point.y >= self.top
            && point.y < self.bottom
    }

    /// Map a point between rectangles, allowing different origins and scales.
    /// Supply the displayed image bounds excluding any letterbox margins.
    pub fn map_point(self, point: Point, target: Rect) -> Option<Point> {
        let (width, height) = self.size()?;
        let (target_width, target_height) = target.size()?;
        self.contains(point).then(|| Point {
            x: target.left + (point.x - self.left) / width * target_width,
            y: target.top + (point.y - self.top) / height * target_height,
        })
    }
}

/// A node in pre-order traversal. Bounds use the snapshot viewport's space.
#[derive(Debug, Clone, PartialEq)]
pub struct Element<K> {
    pub key: Option<K>,
    pub depth: u32,
    pub visible: bool,
    pub bounds: Option<Rect>,
}

/// Provider-defined build and page identifiers that remain stable across sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewIdentity {
    pub build: String,
    pub page: String,
}

/// A parent-to-child path. Anonymous path segments are explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker<K> {
    pub view: ViewIdentity,
    pub path: Vec<Option<K>>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InspectError {
    #[error("viewport must have finite positive dimensions")]
    InvalidViewport,
    #[error("element {0} has no parent at its declared depth")]
    InvalidDepth(usize),
    #[error("marker belongs to a different build or page")]
    DifferentView,
    #[error("element was not found in this snapshot")]
    Missing,
    #[error("more than one element matches the marker path")]
    Ambiguous,
}

pub struct Snapshot<K> {
    view: ViewIdentity,
    viewport: Rect,
    elements: Vec<Element<K>>,
    parents: Vec<Option<usize>>,
}

impl<K: Clone + Eq> Snapshot<K> {
    /// Construct a snapshot from a tree or forest in pre-order.
    pub fn from_preorder(
        view: ViewIdentity,
        viewport: Rect,
        elements: Vec<Element<K>>,
    ) -> Result<Self, InspectError> {
        viewport.size().ok_or(InspectError::InvalidViewport)?;
        let mut ancestors = Vec::new();
        let mut parents = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            let depth = element.depth as usize;
            if depth > ancestors.len() {
                return Err(InspectError::InvalidDepth(index));
            }
            ancestors.truncate(depth);
            parents.push(ancestors.last().copied());
            ancestors.push(index);
        }
        Ok(Self {
            view,
            viewport,
            elements,
            parents,
        })
    }

    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    pub fn element(&self, index: usize) -> Option<&Element<K>> {
        self.elements.get(index)
    }

    /// List geometric candidates ordered by later traversal first. Children
    /// precede their parents. The provider's paint order may differ.
    pub fn hit_candidates(&self, point: Point) -> impl Iterator<Item = usize> + '_ {
        self.elements
            .iter()
            .enumerate()
            .rev()
            .filter_map(move |(index, element)| {
                (self.viewport.contains(point)
                    && element.visible
                    && element.bounds.is_some_and(|bounds| bounds.contains(point)))
                .then_some(index)
            })
    }

    pub fn mark(&self, index: usize) -> Result<Marker<K>, InspectError> {
        let marker = Marker {
            view: self.view.clone(),
            path: self.path(index).ok_or(InspectError::Missing)?,
        };
        self.resolve(&marker)?;
        Ok(marker)
    }

    /// Resolve a marker among the supplied nodes. A filtered or truncated
    /// acquisition can establish uniqueness only within its captured scope.
    pub fn resolve(&self, marker: &Marker<K>) -> Result<usize, InspectError> {
        if marker.view != self.view {
            return Err(InspectError::DifferentView);
        }
        let mut matches = self
            .elements
            .iter()
            .enumerate()
            .filter_map(|(index, element)| {
                let depth = element.depth as usize;
                (depth + 1 == marker.path.len()
                    && marker.path.last() == Some(&element.key)
                    && self.path(index).as_ref() == Some(&marker.path))
                .then_some(index)
            });
        let index = matches.next().ok_or(InspectError::Missing)?;
        if matches.next().is_some() {
            return Err(InspectError::Ambiguous);
        }
        Ok(index)
    }

    fn path(&self, index: usize) -> Option<Vec<Option<K>>> {
        self.elements.get(index)?;
        let mut path = Vec::new();
        let mut current = Some(index);
        while let Some(index) = current {
            path.push(self.elements[index].key.clone());
            current = self.parents[index];
        }
        path.reverse();
        Some(path)
    }
}
