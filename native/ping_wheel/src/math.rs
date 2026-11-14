//! Upstream: common/math/ (1:1).
use crate::compat::Vector4f;
use crate::seams::{self, BlockHitResult, ClipContext, ClipContextBlock, ClipContextFluid, EntityHandle, EntityHitResult, HitResultType, LevelHandle, Matrix4f, Vec2, Vec3};

// ---------------------------------------------------------------------------
// math/ScreenPos.java
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPos {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
}

impl ScreenPos {
    pub fn new(x: f32, y: f32, depth: f32) -> Self {
        Self { x, y, depth }
    }

    pub fn is_behind_camera(&self) -> bool {
        self.depth <= 0.0
    }

    pub fn distance_to(&self, b: Vec2) -> f32 {
        if self.is_behind_camera() {
            return f32::MAX;
        }

        Vec2::new(b.x - self.x, b.y - self.y).length()
    }

    pub fn is_in_bounds(&self, top_left: Vec2, bottom_right: Vec2) -> bool {
        self.x >= top_left.x && self.x <= bottom_right.x && self.y >= top_left.y && self.y <= bottom_right.y
    }
}

// ---------------------------------------------------------------------------
// math/MathUtils.java
// ---------------------------------------------------------------------------

pub struct MathUtils;

impl MathUtils {
    pub fn world_to_screen(world_pos: Vec3, model_view_matrix: &Matrix4f, projection_matrix: &Matrix4f) -> ScreenPos {
        let game = seams::game();
        let window = game.as_ref().map(|g| g.gui_scaled_size()).unwrap_or((0.0, 0.0));
        let camera = game.as_ref().map(|g| g.camera_position()).unwrap_or(Vec3::ZERO);
        let mut world_pos_rel = Vector4f::new(&camera.reverse().add(world_pos), 1.0);
        world_pos_rel = world_pos_rel.mul(model_view_matrix);
        world_pos_rel = world_pos_rel.mul(projection_matrix);

        let depth = world_pos_rel.w;

        if depth != 0.0 {
            world_pos_rel = world_pos_rel.div(depth);
        }

        ScreenPos::new(
            window.0 * (0.5 + world_pos_rel.x * 0.5),
            window.1 * (0.5 - world_pos_rel.y * 0.5),
            depth,
        )
    }

    pub fn rotate_z(matrix_stack: &mut seams::PoseStack, theta: f32) {
        matrix_stack.rotate_z(theta);
    }

    /// Ray/rect edge intersection against the angle direction, ported line-for-line
    /// (f64 internals, Vec2 f32 out; the upstream float/double mix is kept by the casts).
    pub fn calculate_angle_rect_intersection(angle: f32, left_top: Vec2, right_bottom: Vec2) -> Vec2 {
        let angle = angle as f64;
        let direction = Vec3::new(angle.cos(), angle.sin(), 0.0);
        let width = (right_bottom.x - left_top.x) as f64;
        let height = (right_bottom.y - left_top.y) as f64;
        // direction.multiply(new Vec3(1f / width, 1f / height, 0f))
        let direction = Vec3::new(direction.x * (1.0 / width), direction.y * (1.0 / height), 0.0);

        let dx = angle.cos();
        let dy = angle.sin();
        let cx = width * 0.5;
        let cy = height * 0.5;

        if direction.x.abs() < direction.y.abs() {
            if direction.y < 0.0 {
                // top
                let t = -cy / dy;
                let x = cx + t * dx;
                return Vec2::new(x as f32 + left_top.x, left_top.y);
            }

            // bottom
            let t = cy / dy;
            let x = cx + t * dx;
            return Vec2::new(x as f32 + left_top.x, right_bottom.y);
        }

        if direction.x < 0.0 {
            // left
            let t = -cx / dx;
            let y = cy + t * dy;
            return Vec2::new(left_top.x, y as f32 + left_top.y);
        }

        // right
        let t = cx / dx;
        let y = cy + t * dy;
        Vec2::new(right_bottom.x, y as f32 + left_top.y)
    }
}

// ---------------------------------------------------------------------------
// math/Raycast.java
// ---------------------------------------------------------------------------

static TERRAIN_CACHE: std::sync::Mutex<Option<u64>> = std::sync::Mutex::new(None);
/// Upstream starts at Instant.EPOCH, so the first run is always older than 10s and reloads the cache.
static LAST_CACHE_LOAD: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

pub struct Raycast;

impl Raycast {
    /// traceDistantAsync: the DH raycast runs off-thread; the callback fires only on a hit.
    /// Upstream reads CLIENT_CONFIG.getChannel() / pingSequence / GameContext.getDimension()
    /// inside the async body; on a worker thread the host seams default, so the caller
    /// snapshots them here (same values the synchronous path would read).
    pub fn trace_distant_async(direction: Vec3, tick_delta: f32, callback: Box<dyn Fn(BlockHitResult) + Send + 'static>) {
        let Some(camera_entity) = seams::game().and_then(|g| g.camera_entity()) else {
            return;
        };

        let ray_start_vec = camera_entity.position(tick_delta);

        let channel = crate::config::client_handler().config().get_channel();
        let ping_sequence = crate::core::PingController::ping_sequence();
        let dimension = crate::core::GameContext::dimension();

        seams::run_async(Box::new(move || {
            let dh = seams::dh_api();
            let Some(level_wrapper) = dh.single_player_level() else {
                return;
            };

            {
                let mut cache = TERRAIN_CACHE.lock().unwrap();
                let mut last = LAST_CACHE_LOAD.lock().unwrap();
                let expired = match *last {
                    None => true,
                    Some(t) => t.elapsed().as_secs_f64() > 10.0,
                };

                if cache.is_none() || expired {
                    *cache = Some(dh.create_soft_cache());
                    *last = Some(seams::now_instant());
                }

                let cache_token = (*cache).unwrap_or(0);

                let ray_cast_result = dh.raycast(
                    level_wrapper,
                    ray_start_vec.x,
                    ray_start_vec.y,
                    ray_start_vec.z,
                    direction.x as f32,
                    direction.y as f32,
                    direction.z as f32,
                    4096.0,
                    cache_token,
                );

                if !ray_cast_result.success {
                    return;
                }

                if let Some(pos) = ray_cast_result.pos {
                    callback(BlockHitResult {
                        location: pos,
                        block_pos: (pos.x as i32, pos.y as i32, pos.z as i32),
                        direction_up: true,
                        miss_inside: true,
                        hit_type: HitResultType::Block,
                    });
                }
            }
        }));
    }

    /// traceDirectional: block clip vs entity trace, nearer wins.
    pub fn trace_directional(direction: Vec3, tick_delta: f32, max_distance: f64, hit_translucent: bool) -> Option<HitResult> {
        let game = seams::game()?;
        let camera_entity = game.camera_entity()?;
        let level = game.level()?;

        let ray_start_vec = camera_entity.position(tick_delta);
        let ray_end_vec = ray_start_vec.add(direction.scale(max_distance));
        // expandTowards(viewVector(1f).scale(maxDistance)).inflate(1.0, 1.0, 1.0) — host-side AABB.
        let bounding_box = camera_entity.bounding_box_expanded_towards_inflated(camera_entity.view_vector(1.0).scale(max_distance), 1.0);

        let block_hit_result = level.clip(&ClipContext {
            from: ray_start_vec,
            to: ray_end_vec,
            block: if hit_translucent { ClipContextBlock::Outline } else { ClipContextBlock::Visual },
            fluid: if hit_translucent { ClipContextFluid::Any } else { ClipContextFluid::None },
        });

        let entity_hit_result = Raycast::trace_entity(
            camera_entity.clone(),
            ray_start_vec,
            ray_end_vec,
            &bounding_box,
            &|target_entity: &dyn EntityHandle| !target_entity.is_spectator(),
        );

        let Some(entity_hit_result) = entity_hit_result else {
            return block_hit_result.map(HitResult::Block);
        };

        if let Some(block_hit) = block_hit_result {
            if ray_start_vec.distance_to_sqr(block_hit.location) < ray_start_vec.distance_to_sqr(entity_hit_result.location) {
                return Some(HitResult::Block(block_hit));
            }
        }

        Some(HitResult::Entity(entity_hit_result))
    }

    fn trace_entity(
        entity: std::rc::Rc<dyn EntityHandle>,
        min: Vec3,
        max: Vec3,
        box_: &seams::AabbToken,
        predicate: &dyn Fn(&dyn EntityHandle) -> bool,
    ) -> Option<EntityHitResult> {
        let level_token = entity.level_token();
        let level = seams::level_by_token(level_token);
        let mut min_dist = min.distance_to_sqr(max);
        let mut min_hit_result: Option<EntityHitResult> = None;

        for ent in level.get_entities(entity.as_ref(), box_, predicate) {
            let hit_pos = ent.bounding_box_clip(min, max);

            let Some(hit_pos) = hit_pos else {
                continue;
            };

            let hit_result = EntityHitResult { entity_token: ent.uuid() as u64, location: hit_pos };
            let hit_dist = min.distance_to_sqr(hit_result.location);

            if min_dist > hit_dist {
                min_dist = hit_dist;
                min_hit_result = Some(hit_result);
            }
        }

        min_hit_result
    }
}

pub enum HitResult {
    Block(BlockHitResult),
    Entity(EntityHitResult),
}

impl HitResult {
    pub fn get_type(&self) -> HitResultType {
        match self {
            HitResult::Block(_) => HitResultType::Block,
            HitResult::Entity(_) => HitResultType::Entity,
        }
    }

    pub fn get_location(&self) -> Vec3 {
        match self {
            HitResult::Block(b) => b.location,
            HitResult::Entity(e) => e.location,
        }
    }
}
