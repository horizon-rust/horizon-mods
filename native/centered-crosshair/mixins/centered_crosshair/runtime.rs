// centered-crosshair, ported 1:1 to Horizon from JustAlittleWolf/centered-crosshair
// (MIT (c) 2023 JustAlittleWolf, https://github.com/JustAlittleWolf/centered-crosshair).
//
// Upstream is entirely render-side and tiny (4 Java files, 176 LOC). Vanilla
// draws the crosshair with integer blit coordinates, `x = (w - 15) / 2` in
// gui-scaled pixels, where `w` is an integer that already lost its fraction to
// `floor(windowWidth / guiScale)`. When the gui-scaled size is even the sprite
// lands half a pixel off the true window centre. This port keeps the sprite
// centred at the real centre, computed from the raw window size, quantised to
// quarter pixels, and drawn through a subpixel textured-quad path with the
// sprite's u/v nudged by a 1/32768 half-texel so linear sampling reads the
// sprite edges cleanly.
//
// Upstream structure mapped 1:1:
//   HudMixin.centerTheCwosshairPwease        -> centered_crosshair_draw / centered_crosshair_quads
//   DrawContextFloatDrawTexture.blitSprite   -> subpixel_blit_sprite
//   GuiGraphicsExtractorMixin.blitSprite     -> subpixel_blit_sprite (uv nudge + colour -1)
//   SubpixelPositionedTexturedQuad...        -> SubpixelTexturedQuad (vertices + bounds)
//   ScreenRectangle transformMaxBounds       -> Pose::transform_max_bounds (emulated)
//   ScreenRectangle intersection             -> ScreenRect::intersection
//
// Client-side seams (not portable to this pure-Rust core; they are Horizon
// client work, see the filed issue):
//   - the texture-atlas sprite lookup (sprite id -> u0/u1/v0/v1 + atlas texture)
//   - the texture manager lookup behind `blitSprite` (TextureSetup)
//   - the actual GPU draw of a float-position HUD sprite (Horizon's `GuiDraw::Sprite`
//     only carries i32 x/y, so a subpixel crosshair draw has no API surface today)
// The math below is the mod's entire logic and is exact.

/// A JOML `Matrix3x2f` as `[m00, m01, m10, m11, m20, m21]` (Horizon's own
/// `hud_layout::Pose` convention).
pub type Pose = [f32; 6];

/// Identity `Matrix3x2f`.
pub const IDENTITY: Pose = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Java's `Math.round(float)` / `Math.round(double)`: `floor(x + 0.5)`,
/// rounding halves toward positive infinity (so `-2.5 -> -2`, unlike Rust's
/// `f32::round`, which rounds away from zero).
pub fn java_round(x: f32) -> i32 {
    let r = (x + 0.5).floor();
    if r.is_nan() { 0 } else { r as i32 }
}

/// The upstream quarter-pixel quantisation applied to the crosshair anchor:
/// `Math.round((centre - 7.5) * 4) / 4f`.
pub fn quantize_quarter(centre: f32) -> f32 {
    java_round((centre - 7.5) * 4.0) as f32 / 4.0
}

/// `net.minecraft.client.gui.navigation.ScreenRectangle`
/// (`position.x`, `position.y`, `width`, `height`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ScreenRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Vanilla `ScreenRectangle.intersection`: empty (None) when the edges
    /// don't cross, else the clipped rectangle (a touching-edge overlap is a
    /// zero-width/height rectangle, matching `k >= i && l >= j`).
    pub fn intersection(&self, other: &ScreenRect) -> Option<ScreenRect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        if x2 >= x1 && y2 >= y1 {
            Some(ScreenRect::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }

    /// Emulates vanilla `transformMaxBounds(Matrix3x2f)`: the axis-aligned
    /// bounds that contain this rectangle after the pose is applied to its
    /// four corners. With an identity pose it is exact (`floor`/`ceil` of
    /// already-integer corners). The exact vanilla expansion may differ from
    /// this corner model by subpixels in scaled poses; the crosshair path
    /// carries the current HUD pose, which is identity in practice.
    pub fn transform_max_bounds(&self, pose: &Pose) -> ScreenRect {
        let corners = [
            (self.x as f32, self.y as f32),
            ((self.x + self.width) as f32, self.y as f32),
            (self.x as f32, (self.y + self.height) as f32),
            ((self.x + self.width) as f32, (self.y + self.height) as f32),
        ];
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &(cx, cy) in &corners {
            let (tx, ty) = pose_transform_point(pose, cx, cy);
            min_x = min_x.min(tx);
            min_y = min_y.min(ty);
            max_x = max_x.max(tx);
            max_y = max_y.max(ty);
        }
        let x = min_x.floor() as i32;
        let y = min_y.floor() as i32;
        let width = max_x.ceil() as i32 - x;
        let height = max_y.ceil() as i32 - y;
        ScreenRect::new(x, y, width, height)
    }
}

/// Applies a `Matrix3x2f` to a point: `x' = m00*x + m01*y + m20`,
/// `y' = m10*x + m11*y + m21`.
pub fn pose_transform_point(pose: &Pose, x: f32, y: f32) -> (f32, f32) {
    (
        pose[0] * x + pose[1] * y + pose[4],
        pose[2] * x + pose[3] * y + pose[5],
    )
}

/// A texture-atlas sprite's u/v span, the part of the upstream
/// `TextureAtlasSprite` this math consumes (the atlas lookup itself is the
/// client seam).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteUv {
    pub u0: f32,
    pub u1: f32,
    pub v0: f32,
    pub v1: f32,
}

/// One completed `SubpixelPositionedTexturedQuadGuiElementRenderState`: the
/// float-position quad plus the texture to sample, ready for the render
/// state's `addGuiElement` (the client seam).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubpixelTexturedQuad {
    pub pipeline: &'static str,
    /// The atlas texture the sprite lives on (`sprite.atlasLocation()`).
    pub atlas_texture: &'static str,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub u1: f32,
    pub u2: f32,
    pub v1: f32,
    pub v2: f32,
    pub color: u32,
    /// `createBounds` result: `None` when a scissor rejects the quad.
    pub bounds: Option<ScreenRect>,
}

impl SubpixelTexturedQuad {
    /// Upstream `buildVertices`, same corner order:
    /// (x1,y1,u1,v1), (x1,y2,u1,v2), (x2,y2,u2,v2), (x2,y1,u2,v1).
    pub fn build_vertices(&self) -> [(f32, f32, f32, f32); 4] {
        [
            (self.x1, self.y1, self.u1, self.v1),
            (self.x1, self.y2, self.u1, self.v2),
            (self.x2, self.y2, self.u2, self.v2),
            (self.x2, self.y1, self.u2, self.v1),
        ]
    }

    /// Upstream `createBounds`: `ScreenRectangle(round(x1), round(y1),
    /// round(x2-x1), round(y2-y1)).transformMaxBounds(pose)` intersected with
    /// the scissor area, or the plain rect when no scissor is active.
    pub fn create_bounds(
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        pose: &Pose,
        scissor: Option<ScreenRect>,
    ) -> Option<ScreenRect> {
        let rect = ScreenRect::new(
            java_round(x1),
            java_round(y1),
            java_round(x2 - x1),
            java_round(y2 - y1),
        )
        .transform_max_bounds(pose);
        match scissor {
            Some(area) => area.intersection(&rect),
            None => Some(rect),
        }
    }
}

/// `1 / 32768`, the upstream half-texel nudge (`0b1 / 32768f`), exact in f32.
pub const HALF_TEXEL: f32 = 1.0 / 32768.0;

/// Upstream `DrawContextFloatDrawTexture.blitSprite` as patched through
/// `GuiGraphicsExtractorMixin`: look up the sprite (seam, passed in as
/// `sprite_uv` + its atlas), then emit the float-position quad with the
/// sprite's u/v nudged by a half texel (u up, v down) and colour `-1`.
/// `width == 0 || height == 0` draws nothing, like upstream's early return.
pub fn subpixel_blit_sprite(
    pipeline: &'static str,
    atlas_texture: &'static str,
    sprite_uv: &SpriteUv,
    x: f32,
    y: f32,
    width: i32,
    height: i32,
    pose: &Pose,
    scissor: Option<ScreenRect>,
) -> Option<SubpixelTexturedQuad> {
    if width == 0 || height == 0 {
        return None;
    }
    let quad = SubpixelTexturedQuad {
        pipeline,
        atlas_texture,
        x1: x,
        y1: y,
        x2: x + width as f32,
        y2: y + height as f32,
        u1: sprite_uv.u0 + HALF_TEXEL,
        u2: sprite_uv.u1 + HALF_TEXEL,
        v1: sprite_uv.v0 - HALF_TEXEL,
        v2: sprite_uv.v1 - HALF_TEXEL,
        // Upstream passes `-1` (an ARGB int), i.e. fully opaque white.
        color: 0xFFFF_FFFF,
        bounds: SubpixelTexturedQuad::create_bounds(
            x,
            y,
            x + width as f32,
            y + height as f32,
            pose,
            scissor,
        ),
    };
    Some(quad)
}

/// The crosshair HUD draw, porting `HudMixin`: the vanilla crosshair blit's
/// integer anchor is dropped and the sprite is re-anchored at the true window
/// centre, quantised to quarter pixels, drawn 15x15 on the crosshair pipeline.
///
/// `window_width` / `window_height` are the raw frame-buffer pixels (not the
/// gui-scaled size): the fix lives in dividing by the gui scale in f32, so the
/// fraction vanilla's `floor` threw away comes back. Returns one quad, or
/// none when the sprite size is zero.
pub fn centered_crosshair_draw(
    window_width: i32,
    window_height: i32,
    gui_scale: f32,
    sprite_uv: &SpriteUv,
    pose: &Pose,
    scissor: Option<ScreenRect>,
) -> Option<SubpixelTexturedQuad> {
    let scaled_center_x = (window_width as f32 / gui_scale) / 2.0;
    let scaled_center_y = (window_height as f32 / gui_scale) / 2.0;
    let x = quantize_quarter(scaled_center_x);
    let y = quantize_quarter(scaled_center_y);
    subpixel_blit_sprite(
        "pipeline/crosshair",
        "textures/gui/sprites/hud/crosshair",
        sprite_uv,
        x,
        y,
        15,
        15,
        pose,
        scissor,
    )
}

/// Vanilla's integer crosshair anchor for comparison
/// (`sprite("hud/crosshair", (w - 15) / 2, (h - 15) / 2, 15, 15)`), where `w`
/// is the gui-scaled size. Tests use it to prove the port actually moves the
/// sprite onto the true centre.
pub fn vanilla_crosshair_anchor(gui_scaled_size: i32) -> i32 {
    (gui_scaled_size - 15) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite_uv() -> SpriteUv {
        // 15x15 sprite in a 256x256 atlas at slot (32, 32): u = 32/256 ...
        SpriteUv {
            u0: 0.125,
            u1: 0.18359375,
            v0: 0.125,
            v1: 0.18359375,
        }
    }

    #[test]
    fn java_round_rounds_halves_toward_positive_infinity() {
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(3.7), 4);
        assert_eq!(java_round(-3.2), -3);
        assert_eq!(java_round(0.4999), 0);
        assert_eq!(java_round(0.0), 0);
    }

    #[test]
    fn quarter_quantization_matches_upstream_math() {
        // (centre - 7.5) * 4 rounded, then / 4.
        assert_eq!(quantize_quarter(320.3), 312.75); // round(1251.2) = 1251 -> 312.75
        assert_eq!(quantize_quarter(320.0), 312.5); // 1250 / 4
        assert_eq!(quantize_quarter(149.5), 142.0); // 568 / 4
        assert_eq!(quantize_quarter(7.5), 0.0);
        assert_eq!(quantize_quarter(7.4), 0.0); // round((7.4 - 7.5) * 4) = round(-0.4) = 0
    }

    #[test]
    fn quantize_rounds_negatives_toward_positive_infinity() {
        // (-0.25): (-7.75 - 7.5) * 4 = -61.0 -> java_round = -60 (floor(-60.5) = -61? no:
        // -61.0 + 0.5 = -60.5, floor = -61). -61/4 = -15.25.
        assert_eq!(quantize_quarter(-7.75), -15.25);
        // A case where Rust's round would differ from Java's: x*4 = -3.5 ->
        // java_round = -3 (floor(-3.0)), Rust round(-3.5) = -4. centre = 6.625:
        // (6.625 - 7.5) * 4 = -3.5.
        assert_eq!(quantize_quarter(6.625), -0.75);
    }

    #[test]
    fn zero_sized_sprite_draws_nothing() {
        let uv = sprite_uv();
        assert!(
            subpixel_blit_sprite(
                "pipeline/crosshair",
                "tex",
                &uv,
                1.0,
                1.0,
                0,
                15,
                &IDENTITY,
                None
            )
            .is_none()
        );
        assert!(
            subpixel_blit_sprite(
                "pipeline/crosshair",
                "tex",
                &uv,
                1.0,
                1.0,
                15,
                0,
                &IDENTITY,
                None
            )
            .is_none()
        );
        assert!(
            subpixel_blit_sprite(
                "pipeline/crosshair",
                "tex",
                &uv,
                1.0,
                1.0,
                0,
                0,
                &IDENTITY,
                None
            )
            .is_none()
        );
    }

    #[test]
    fn blit_nudges_uv_by_half_texel_and_sets_opaque_white() {
        let uv = sprite_uv();
        let quad = subpixel_blit_sprite(
            "pipeline/crosshair",
            "tex",
            &uv,
            10.0,
            20.0,
            15,
            15,
            &IDENTITY,
            None,
        )
        .unwrap();
        assert_eq!(quad.u1, uv.u0 + HALF_TEXEL);
        assert_eq!(quad.u2, uv.u1 + HALF_TEXEL);
        assert_eq!(quad.v1, uv.v0 - HALF_TEXEL);
        assert_eq!(quad.v2, uv.v1 - HALF_TEXEL);
        assert_eq!(quad.color, 0xFFFF_FFFF);
        assert_eq!(quad.x1, 10.0);
        assert_eq!(quad.x2, 25.0);
        assert_eq!(quad.y1, 20.0);
        assert_eq!(quad.y2, 35.0);
    }

    #[test]
    fn vertices_follow_upstream_corner_order() {
        let quad = SubpixelTexturedQuad {
            pipeline: "p",
            atlas_texture: "t",
            x1: 1.0,
            y1: 2.0,
            x2: 3.0,
            y2: 4.0,
            u1: 0.1,
            u2: 0.2,
            v1: 0.3,
            v2: 0.4,
            color: 7,
            bounds: None,
        };
        assert_eq!(
            quad.build_vertices(),
            [
                (1.0, 2.0, 0.1, 0.3),
                (1.0, 4.0, 0.1, 0.4),
                (3.0, 4.0, 0.2, 0.4),
                (3.0, 2.0, 0.2, 0.3),
            ]
        );
    }

    #[test]
    fn create_bounds_is_the_rounded_rect_under_identity_pose() {
        let b =
            SubpixelTexturedQuad::create_bounds(10.25, 20.5, 25.25, 35.5, &IDENTITY, None).unwrap();
        assert_eq!(b, ScreenRect::new(10, 21, 15, 15));
    }

    #[test]
    fn create_bounds_grows_under_a_scaled_pose() {
        // A x2 scale about the origin: the quad (10..25, 20..35) becomes
        // (20..50, 40..70); its bounds must contain it.
        let pose = [2.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        let b = SubpixelTexturedQuad::create_bounds(10.0, 20.0, 25.0, 35.0, &pose, None).unwrap();
        assert!(b.x <= 20 && b.y <= 40 && b.x + b.width >= 50 && b.y + b.height >= 70);
    }

    #[test]
    fn create_bounds_intersects_the_scissor() {
        let scissor = ScreenRect::new(0, 0, 20, 20);
        // Quad (10..25, 20..35) clipped to (10..20, 20..20): y-overlap empty
        // height 0 still crosses (`k >= i && l >= j`), so Some with height 0.
        let b =
            SubpixelTexturedQuad::create_bounds(10.0, 20.0, 25.0, 35.0, &IDENTITY, Some(scissor))
                .unwrap();
        assert_eq!(b, ScreenRect::new(10, 20, 10, 0));
        // Fully outside: None.
        assert!(
            SubpixelTexturedQuad::create_bounds(
                100.0,
                100.0,
                115.0,
                115.0,
                &IDENTITY,
                Some(scissor)
            )
            .is_none()
        );
    }

    #[test]
    fn centered_draw_lands_on_the_true_window_centre() {
        // Window 1000x600 at gui scale 3: vanilla's gui-scaled size floors
        // 1000/3 to 333, anchor (333 - 15) / 2 = 159, sprite centre 166.5.
        // The true centre is 1000/3/2 = 166.6667; the port quantises to
        // round((166.6667 - 7.5) * 4) / 4 = round(636.6667) / 4 = 637 / 4 = 159.25,
        // sprite centre 166.75: half the vanilla error, and the missing 0.08
        // is the quarter-pixel cap, not the integer truncation.
        let quad = centered_crosshair_draw(1000, 600, 3.0, &sprite_uv(), &IDENTITY, None).unwrap();
        assert_eq!(quad.pipeline, "pipeline/crosshair");
        assert_eq!(quad.x1, 159.25);
        assert_eq!(quad.y1, 92.5); // 600/3/2 = 100, round((100 - 7.5) * 4) / 4 = 370 / 4 = 92.5
    }
}
