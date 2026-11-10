//! Declarative content shared by LearningCards and Questions.
//!
//! Phase 1 renders text-like blocks as plain text. The remaining variants are
//! schema-only extension points for Phase 2. Content is data, never executable
//! HTML or JavaScript.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Heading {
        level: u8,
        text: String,
    },
    Text {
        text: String,
    },
    Markdown {
        markdown: String,
    },
    Math {
        latex: String,
        #[serde(default)]
        display: bool,
    },
    Table {
        #[serde(default)]
        caption: Option<String>,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Image {
        asset: String,
        alt: String,
        /// Optional crop/annotation window in normalized 0..1 coordinates.
        #[serde(default)]
        normalized_rect: Option<NormalizedRect>,
    },
    Flowchart {
        /// A deliberately limited, non-executable Mermaid-like subset.
        source: String,
    },
    Diagram(DiagramSpec),
    Graph(GraphSpec),
    Geometry(GeometrySpec),
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn heading(level: u8, text: impl Into<String>) -> Self {
        Self::Heading {
            level,
            text: text.into(),
        }
    }

    pub fn markdown(markdown: impl Into<String>) -> Self {
        Self::Markdown {
            markdown: markdown.into(),
        }
    }

    pub fn plain_text(&self) -> String {
        match self {
            Self::Heading { text, .. } | Self::Text { text } => text.clone(),
            Self::Markdown { markdown } => markdown.clone(),
            Self::Math { latex, .. } => latex.clone(),
            Self::Table {
                caption,
                headers,
                rows,
            } => {
                let mut lines = Vec::new();
                if let Some(caption) = caption {
                    lines.push(caption.clone());
                }
                lines.push(headers.join(" | "));
                lines.extend(rows.iter().map(|row| row.join(" | ")));
                lines.join("\n")
            }
            Self::Image { alt, .. } => format!("[画像] {alt}"),
            Self::Flowchart { .. } => "[フローチャート]".to_owned(),
            Self::Diagram(_) => "[図]".to_owned(),
            Self::Graph(_) => "[グラフ]".to_owned(),
            Self::Geometry(_) => "[図形]".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// Geometry / graph / diagram data. Display coordinates are explicit and are
// not used as mathematical truth. Validation arrives in Phase 2.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagramSpec {
    pub width: u32,
    pub height: u32,
    pub elements: Vec<DiagramElement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiagramElement {
    Line {
        from: NormalizedPoint,
        to: NormalizedPoint,
        #[serde(default)]
        dashed: bool,
    },
    Arrow {
        from: NormalizedPoint,
        to: NormalizedPoint,
        #[serde(default)]
        dashed: bool,
    },
    Rect {
        rect: NormalizedRect,
        #[serde(default)]
        label: Option<String>,
    },
    Circle {
        center: NormalizedPoint,
        radius: f32,
        #[serde(default)]
        label: Option<String>,
    },
    Label {
        at: NormalizedPoint,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphSpec {
    pub x_range: [f64; 2],
    pub y_range: [f64; 2],
    #[serde(default)]
    pub grid: bool,
    pub elements: Vec<GraphElement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphElement {
    Line {
        slope: f64,
        intercept: f64,
        #[serde(default)]
        label: Option<String>,
    },
    Segment {
        from: [f64; 2],
        to: [f64; 2],
    },
    Point {
        at: [f64; 2],
        #[serde(default)]
        label: Option<String>,
    },
    /// Sampled function points; the UI draws a polyline, not executable code.
    Curve {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        label: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometrySpec {
    pub points: Vec<GeoPoint>,
    pub elements: Vec<GeoElement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub id: String,
    /// Display coordinate only. Mathematical constraints live in `elements`.
    pub at: NormalizedPoint,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeoElement {
    Segment {
        from: String,
        to: String,
        #[serde(default)]
        dashed: bool,
    },
    Polygon {
        points: Vec<String>,
        #[serde(default)]
        filled: bool,
    },
    Circle {
        center: String,
        radius: f64,
    },
    AngleArc {
        vertex: String,
        from: String,
        to: String,
        #[serde(default)]
        degrees: Option<f64>,
    },
    RightAngle {
        vertex: String,
        from: String,
        to: String,
    },
    ParallelMarks {
        segments: Vec<[String; 2]>,
    },
    EqualMarks {
        segments: Vec<[String; 2]>,
    },
    DimensionLine {
        from: String,
        to: String,
        text: String,
    },
    Label {
        at: String,
        text: String,
    },
    /// Semantic constraints used by validation; never rendered directly.
    Constraint {
        kind: String,
        refs: Vec<String>,
        #[serde(default)]
        value: Option<f64>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HintOverlay {
    pub blocks: Vec<ContentBlock>,
    /// Optional geometry/graph highlight expressed as data.
    #[serde(default)]
    pub highlight_points: Vec<NormalizedPoint>,
    #[serde(default)]
    pub highlight_segments: Vec<[NormalizedPoint; 2]>,
    #[serde(default)]
    pub note: Option<String>,
}
