use malilib::util::{BlockPos, LayerRange};
use crate::placement::SchematicPlacement;
use crate::schematic::BlockState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MismatchType {
    MissingBlock { expected: BlockState, found: BlockState },
    ExtraBlock { expected: BlockState, found: BlockState },
    WrongBlock { expected: BlockState, found: BlockState },
    WrongState { expected: BlockState, found: BlockState },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MismatchEntry {
    pub pos: BlockPos,
    pub mismatch: MismatchType,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VerificationResult {
    pub total_checked: usize,
    pub correct_count: usize,
    pub missing_count: usize,
    pub extra_count: usize,
    pub wrong_block_count: usize,
    pub wrong_state_count: usize,
    pub mismatches: Vec<MismatchEntry>,
}

impl VerificationResult {
    pub fn is_clean(&self) -> bool {
        self.mismatches.is_empty()
    }

    pub fn total_mismatches(&self) -> usize {
        self.mismatches.len()
    }
}

pub struct SchematicVerifier;

impl SchematicVerifier {
    pub fn verify_placement<F: Fn(BlockPos) -> BlockState>(
        placement: &SchematicPlacement,
        layer_range: Option<&LayerRange>,
        get_world_block: F,
    ) -> VerificationResult {
        let mut result = VerificationResult::default();
        let enc_box = placement.get_enclosing_world_box();

        for world_pos in enc_box.iter_blocks() {
            if let Some(lr) = layer_range {
                if !lr.is_position_within(world_pos) {
                    continue;
                }
            }

            let expected_opt = placement.get_block_at_world_pos(world_pos);
            let found = get_world_block(world_pos);

            let expected = expected_opt.unwrap_or_else(|| BlockState::new(BlockState::AIR));

            if expected.is_air() && found.is_air() {
                continue;
            }

            result.total_checked += 1;

            if expected == found {
                result.correct_count += 1;
            } else if found.is_air() && !expected.is_air() {
                result.missing_count += 1;
                result.mismatches.push(MismatchEntry {
                    pos: world_pos,
                    mismatch: MismatchType::MissingBlock { expected, found },
                });
            } else if !found.is_air() && expected.is_air() {
                result.extra_count += 1;
                result.mismatches.push(MismatchEntry {
                    pos: world_pos,
                    mismatch: MismatchType::ExtraBlock { expected, found },
                });
            } else if expected.name != found.name {
                result.wrong_block_count += 1;
                result.mismatches.push(MismatchEntry {
                    pos: world_pos,
                    mismatch: MismatchType::WrongBlock { expected, found },
                });
            } else {
                result.wrong_state_count += 1;
                result.mismatches.push(MismatchEntry {
                    pos: world_pos,
                    mismatch: MismatchType::WrongState { expected, found },
                });
            }
        }

        result
    }
}
