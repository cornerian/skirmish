//! Pure mapping between SDL window coordinates, surface pixels, and authored UI space.

/// The coordinate extent authored by Melee's main-menu camera and scissor.
pub const MELEE_AUTHORED_EXTENT: [f32; 2] = [640.0, 480.0];

/// A snapshot of the geometry used to present an authored canvas in a window.
///
/// SDL pointer events use window coordinates, which can differ from drawable
/// pixels on a high-density display. Rendering first scales into surface pixels,
/// then fits the authored aspect ratio inside that surface. Keeping both steps in
/// this value prevents pointer hit testing from drifting away from presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentationTransform {
    window_extent: [u32; 2],
    surface_extent: [u32; 2],
    authored_extent: [f32; 2],
    surface_viewport: [f32; 4],
}

impl PresentationTransform {
    /// Build a transform from SDL window units to the current surface and canvas.
    ///
    /// Returns `None` for zero-sized windows or surfaces and for invalid authored
    /// dimensions. The surface extent should be the actual configured swapchain
    /// extent, not an assumed pixel-density multiple.
    pub fn new(
        window_extent: [u32; 2],
        surface_extent: [u32; 2],
        authored_extent: [f32; 2],
    ) -> Option<Self> {
        if window_extent.contains(&0)
            || authored_extent
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return None;
        }
        let surface_viewport =
            fitted_viewport(surface_extent, authored_extent[0] / authored_extent[1])?;
        Some(Self {
            window_extent,
            surface_extent,
            authored_extent,
            surface_viewport,
        })
    }

    /// The `[x, y, width, height]` viewport in configured surface pixels.
    pub const fn surface_viewport(self) -> [f32; 4] {
        self.surface_viewport
    }

    /// Convert an SDL pointer position into authored coordinates.
    ///
    /// The active viewport uses half-open bounds: its top and left edges belong
    /// to the canvas, while its right and bottom edges (and all bars) do not.
    pub fn window_to_authored(self, point: [f32; 2]) -> Option<[f32; 2]> {
        if point.iter().any(|value| !value.is_finite()) {
            return None;
        }
        let window = [self.window_extent[0] as f32, self.window_extent[1] as f32];
        if point[0] < 0.0 || point[0] >= window[0] || point[1] < 0.0 || point[1] >= window[1] {
            return None;
        }
        self.surface_to_authored([
            point[0] * self.surface_extent[0] as f32 / window[0],
            point[1] * self.surface_extent[1] as f32 / window[1],
        ])
    }

    fn surface_to_authored(self, point: [f32; 2]) -> Option<[f32; 2]> {
        let [x, y, width, height] = self.surface_viewport;
        if point[0] < x || point[0] >= x + width || point[1] < y || point[1] >= y + height {
            return None;
        }
        Some([
            (point[0] - x) * self.authored_extent[0] / width,
            (point[1] - y) * self.authored_extent[1] / height,
        ])
    }
}

/// Fit an aspect ratio inside a surface, returning `[x, y, width, height]` in pixels.
pub fn fitted_viewport(surface_extent: [u32; 2], authored_aspect: f32) -> Option<[f32; 4]> {
    if surface_extent.contains(&0) || !authored_aspect.is_finite() || authored_aspect <= 0.0 {
        return None;
    }
    let width = surface_extent[0] as f32;
    let height = surface_extent[1] as f32;
    if width / height > authored_aspect {
        let viewport_width = height * authored_aspect;
        Some([(width - viewport_width) * 0.5, 0.0, viewport_width, height])
    } else {
        let viewport_height = width / authored_aspect;
        Some([
            0.0,
            (height - viewport_height) * 0.5,
            width,
            viewport_height,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_point(actual: Option<[f32; 2]>, expected: [f32; 2]) {
        let actual = actual.expect("point should be inside the authored viewport");
        assert!(
            actual
                .into_iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() < 0.001),
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn fitted_viewports_cover_four_three_wide_and_square_surfaces() {
        for (surface, expected) in [
            ([640, 480], [0.0, 0.0, 640.0, 480.0]),
            ([1280, 720], [160.0, 0.0, 960.0, 720.0]),
            ([640, 640], [0.0, 80.0, 640.0, 480.0]),
        ] {
            let actual = fitted_viewport(surface, 4.0 / 3.0).unwrap();
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(actual, expected)| (actual - expected).abs() < 0.001),
                "{actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn four_three_mapping_has_half_open_boundaries() {
        let transform =
            PresentationTransform::new([640, 480], [640, 480], MELEE_AUTHORED_EXTENT).unwrap();
        assert_eq!(transform.surface_viewport(), [0.0, 0.0, 640.0, 480.0]);
        assert_point(transform.window_to_authored([0.0, 0.0]), [0.0, 0.0]);
        assert_point(transform.window_to_authored([320.0, 240.0]), [320.0, 240.0]);
        assert_point(
            transform.window_to_authored([639.999, 479.999]),
            [639.999, 479.999],
        );
        for point in [
            [-0.001, 0.0],
            [0.0, -0.001],
            [640.0, 0.0],
            [0.0, 480.0],
            [f32::NAN, 0.0],
            [0.0, f32::INFINITY],
        ] {
            assert_eq!(transform.window_to_authored(point), None, "{point:?}");
        }
    }

    #[test]
    fn wide_and_square_bars_are_not_interactive() {
        let wide =
            PresentationTransform::new([1280, 720], [1280, 720], MELEE_AUTHORED_EXTENT).unwrap();
        for point in [[0.0, 360.0], [159.999, 360.0], [1120.0, 360.0]] {
            assert_eq!(wide.window_to_authored(point), None, "{point:?}");
        }
        assert_point(wide.window_to_authored([160.0, 0.0]), [0.0, 0.0]);
        assert_point(wide.window_to_authored([640.0, 360.0]), [320.0, 240.0]);

        let square =
            PresentationTransform::new([640, 640], [640, 640], MELEE_AUTHORED_EXTENT).unwrap();
        for point in [[320.0, 0.0], [320.0, 79.999], [320.0, 560.0]] {
            assert_eq!(square.window_to_authored(point), None, "{point:?}");
        }
        assert_point(square.window_to_authored([0.0, 80.0]), [0.0, 0.0]);
        assert_point(square.window_to_authored([320.0, 320.0]), [320.0, 240.0]);
    }

    #[test]
    fn high_density_and_fractional_density_preserve_authored_coordinates() {
        for (window, surface, point, expected) in [
            ([640, 480], [1280, 960], [320.0, 240.0], [320.0, 240.0]),
            ([800, 600], [1200, 900], [200.0, 150.0], [160.0, 120.0]),
            ([1280, 720], [2560, 1440], [640.0, 360.0], [320.0, 240.0]),
        ] {
            let transform =
                PresentationTransform::new(window, surface, MELEE_AUTHORED_EXTENT).unwrap();
            assert_point(transform.window_to_authored(point), expected);
        }
    }

    #[test]
    fn mapping_uses_the_configured_surface_instead_of_assuming_pixel_density() {
        let transform =
            PresentationTransform::new([1600, 900], [1000, 800], MELEE_AUTHORED_EXTENT).unwrap();
        assert_eq!(transform.surface_viewport(), [0.0, 25.0, 1000.0, 750.0]);
        assert_point(transform.window_to_authored([800.0, 450.0]), [320.0, 240.0]);
        assert_eq!(transform.window_to_authored([800.0, 28.124]), None);
        assert_point(transform.window_to_authored([0.0, 28.125]), [0.0, 0.0]);
    }

    #[test]
    fn invalid_extents_and_aspects_are_rejected() {
        for window in [[0, 480], [640, 0]] {
            assert_eq!(
                PresentationTransform::new(window, [640, 480], MELEE_AUTHORED_EXTENT),
                None
            );
        }
        for surface in [[0, 480], [640, 0]] {
            assert_eq!(
                PresentationTransform::new([640, 480], surface, MELEE_AUTHORED_EXTENT),
                None
            );
        }
        for authored in [
            [0.0, 480.0],
            [640.0, 0.0],
            [-640.0, 480.0],
            [f32::NAN, 480.0],
            [640.0, f32::INFINITY],
        ] {
            assert_eq!(
                PresentationTransform::new([640, 480], [640, 480], authored),
                None
            );
        }
        for aspect in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(fitted_viewport([640, 480], aspect), None);
        }
    }
}
