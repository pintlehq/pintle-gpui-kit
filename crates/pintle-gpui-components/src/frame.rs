//! Paint helpers for opaque, one-pixel framed regions.
use gpui::{
    AnyElement, App, Axis, Bounds, ContentMask, Element, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, PathBuilder, Pixels, Styled, Window, canvas, point, px,
};
use gpui_kit as gpui;
use pintle_gpui_theme::ActiveTheme;
use std::rc::Rc;

/// Cover rectangular child painting outside a rounded frame, then repaint its
/// one-pixel outline. Add to a relative, overflow-hidden container with a
/// transparent one-pixel border so the outline is painted only once.
/// `surround` must match the opaque surface outside the frame. This is a
/// hitbox-free overlay below Kit popovers/tooltips; it does not change layout.
/// GPUI's overflow mask is rectangular even when the container is rounded.
pub fn rounded_frame_overlay(radius: Pixels, surround: Hsla, border: Hsla) -> impl IntoElement {
    FrameOverlay {
        child: Some(
            canvas(
                |bounds, _, _| bounds.dilate(px(1.)),
                move |_, bounds, window, _| {
                    let radius = radius
                        .max(px(0.))
                        .min(bounds.size.width / 2.)
                        .min(bounds.size.height / 2.);
                    let r = radius.as_f32();
                    // Cubic approximation of a circular quarter, reflected for each corner.
                    const K: f32 = 0.552_284_8;
                    for (right, bottom) in
                        [(false, false), (true, false), (false, true), (true, true)]
                    {
                        let at = |x: f32, y: f32| {
                            point(
                                if right {
                                    bounds.right() - px(x)
                                } else {
                                    bounds.left() + px(x)
                                },
                                if bottom {
                                    bounds.bottom() - px(y)
                                } else {
                                    bounds.top() + px(y)
                                },
                            )
                        };
                        let mut path = PathBuilder::fill();
                        path.move_to(at(0., 0.));
                        path.line_to(at(r, 0.));
                        path.cubic_bezier_to(at(0., r), at(r * (1. - K), 0.), at(0., r * (1. - K)));
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, surround);
                        }
                    }
                    window.paint_quad(gpui::quad(
                        bounds,
                        radius,
                        gpui::transparent_black(),
                        px(1.),
                        border,
                        Default::default(),
                    ));
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        ),
    }
}

// Unlike an unbounded deferred overlay, retain the ancestor's clipping when a
// split is narrower/shorter than its content. Restore the one-pixel border only
// on edges belonging to this frame, never on an edge clipped by an ancestor.
struct FrameOverlay {
    child: Option<AnyElement>,
}
impl IntoElement for FrameOverlay {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FrameOverlay {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<gpui::ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.as_mut().unwrap().request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let mask = window.content_mask().bounds;
        let clip = Bounds::from_corners(
            point(
                if mask.left() <= bounds.left() {
                    bounds.left() - px(1.)
                } else {
                    mask.left()
                },
                if mask.top() <= bounds.top() {
                    bounds.top() - px(1.)
                } else {
                    mask.top()
                },
            ),
            point(
                if mask.right() >= bounds.right() {
                    bounds.right() + px(1.)
                } else {
                    mask.right()
                },
                if mask.bottom() >= bounds.bottom() {
                    bounds.bottom() + px(1.)
                } else {
                    mask.bottom()
                },
            ),
        );
        window.defer_draw(
            self.child.take().unwrap(),
            window.element_offset(),
            0,
            Some(ContentMask { bounds: clip }),
        );
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

/// A framed region already supplies its border. Keep the Kit resize hit area,
/// but paint only a short handle on hover/drag, not a second full-length line.
pub fn frame_resize_handle_appearance() -> gpui::base::ResizeHandleRenderer {
    Rc::new(|handle, _, cx| {
        let active = handle.state() != gpui::base::ResizeHandleState::Idle;
        let axis = handle.axis();
        let color = cx.theme().ring;
        Some(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, window, _| {
                    if !active {
                        return;
                    }
                    let size = if axis == Axis::Horizontal {
                        gpui::size(px(3.), px(24.))
                    } else {
                        gpui::size(px(24.), px(3.))
                    };
                    let rect = gpui::Bounds::new(
                        bounds.center() - point(size.width / 2., size.height / 2.),
                        size,
                    );
                    window.paint_quad(gpui::quad(
                        rect,
                        px(1.5),
                        color,
                        px(0.),
                        color,
                        Default::default(),
                    ));
                },
            )
            .size_full()
            .into_any_element(),
        )
    })
}
