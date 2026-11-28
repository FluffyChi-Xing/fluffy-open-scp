//! Ground low-wealth, high-density residence from 48E0C000:EBCE40EB.
//! Only the residential simulation bindings transfer to a ploppable component.

use sc_properties::Value;

pub(super) const PROFILE: &str = "ground-residence-w1-d3-v1";
pub(super) const POPULATION: [u32; 3] = [600, 400, 100];
pub(super) const INTAKE: u32 = 0x999F_8484;
pub(super) const TAXES: u32 = 0x927C_3F79;
pub(super) const MOVE_IN_SINK: u32 = 0xBBAA_D3D1;

// Common W1 living rules are inherited from 48E0C000:EC561808 in the
// original; this component declares them alongside the D3 ground bindings.
pub(super) const BUNDLES: &[u32] = &[
    0xD4C3_0C02,
    0x7378_2B22,
    0x568C_CCA3,
    0xCC2A_40B6,
    0xE648_41C9,
    0x7C37_AF27,
    0x4E7D_5743,
    0xBAC9_6038,
    0x3B72_84AB,
    0xEE66_978E,
    0x351E_43C1,
    INTAKE,
    TAXES,
];
pub(super) const GROUND: &[u32] = &[0x5D5E_F1FF, 0xA3F5_54BD, 0xE238_1694, 0xCC2B_572E];
// EBCE40EB inherits the zoning construction initializer from A8210868.
// Copan 4DC31F9B uses its plop state bundle with a vacant initial state.
pub(super) const GROWTH_STATE: u32 = 0x182D_2924;
pub(super) const PLOP_STATE: u32 = 0x933F_0FBF;
pub(super) const ALTERNATE: &[u32] = &[0xCC2B_572F, GROWTH_STATE];

pub(super) fn blocks_move_in(flag: &Value) -> bool {
    matches!(flag, Value::Int32(13 | 14))
}

pub(super) const DENSITY: u32 = 0x192F_CF56;
pub(super) const FIELDS: &[(u32, f32)] = &[
    (0x0A79_17C4, 200.0),
    (0x0C20_0553, 3.0),
    (0x0E54_6792, 66000.0),
    (0x0D7B_1448, 0.12),
    (0x0D7B_1449, 0.30),
    (0x0D7B_144A, 0.50),
    (0x0D7B_144B, 1.0),
    (0x0D63_43C2, 0.56),
    (0x0D63_5C98, 243.0),
    (0x0D89_7622, 60.0),
];
