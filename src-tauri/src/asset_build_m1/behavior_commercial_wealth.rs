//! Ground retail records selected from the effective D1 templates
//! 97E741B3 / 66549E0C / 7F496E35. See docs/re/commercial-wealth-recipes-20261114.md.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(super) enum RetailWealth {
    W1,
    W2,
    W3,
}

pub(super) struct RetailProfile {
    pub jobs: [i32; 3],
    pub shoppers: i32,
    pub goods: i32,
    pub slots: [i32; 3],
    pub shifts: [i32; 8],
    pub population: f32,
    pub labor_tuning: f32,
    pub profit: f32,
    pub vacancy_tuning: f32,
    pub category: u32,
    pub unit_kind: u32,
    pub bundles: [u32; 4],
}

// Common ground freight (94480760, inheriting BE47F5A2), profit,
// auxiliary shoppers, and the explicit ground labor provider.
const COMMON: [u32; 7] = [
    0x2B5A_2BAF,
    0x3620_87BD,
    0x9448_0760,
    0xD07C_5A04,
    0xF236_BB93,
    0x3748_6456,
    0x7424_049F,
];
const OTHER_PROVIDERS: &[u32] = &[
    0xBE47_F5A2,
    0x3A82_63E0,
    0xBC76_13A3,
    0x3064_5625,
    0x1E16_EDA9,
    0x5D5E_F1FF,
    0x5D5E_F1FE,
    0x6EA8_E701,
    0x265F_B8DD,
    0x265F_B8DE,
    0x265F_B8DF,
    0xC067_2F54,
    0xC067_2F55,
    0xC067_2F56,
    0xC067_2F57,
    0x5FFA_86B9,
    0xEA02_54BF,
    0xE153_69A2,
    0x75D2_2FD2,
    0x75D2_2FD1,
    0x75D2_2FD0,
    0x729A_6626,
    0x1C1F_5978,
    0x1C1F_597B,
    0x1C1F_597A,
    0x8A72_31A2,
    0x8A72_31A1,
    0x8A72_31A0,
];

impl RetailWealth {
    pub(super) fn select(params: &serde_json::Value) -> Result<Option<Self>, String> {
        match &params["commercialProfile"] {
            serde_json::Value::Null => Ok(None),
            value => match value.as_str() {
                Some("ground-commercial-w1-v1") => Ok(Some(Self::W1)),
                Some("ground-commercial-w2-v1") => Ok(Some(Self::W2)),
                Some("ground-commercial-w3-v1") => Ok(Some(Self::W3)),
                _ => Err("COMMERCIAL_PARAMETER: commercialProfile".into()),
            },
        }
    }

    pub(super) fn profile(self) -> RetailProfile {
        match self {
            Self::W1 => RetailProfile {
                jobs: [5, 2, 0],
                shoppers: 10,
                goods: 21,
                slots: [4, 0, 0],
                shifts: [0, 0, 6, 6, 6, 6, 0, 0],
                population: 14.0,
                labor_tuning: 8.0,
                profit: 112.0,
                vacancy_tuning: 20.0,
                category: 0x7F2E_A045,
                unit_kind: 0xCAF1_511E,
                bundles: [0x3A82_63E1, 0x6827_D125, 0xC3EC_8B4E, 0xF25B_DF56],
            },
            Self::W2 => RetailProfile {
                jobs: [3, 2, 1],
                shoppers: 8,
                goods: 18,
                slots: [2, 1, 0],
                shifts: [0, 0, 5, 5, 4, 4, 0, 0],
                population: 12.0,
                labor_tuning: 6.0,
                profit: 96.0,
                vacancy_tuning: 60.0,
                category: 0x7136_D91E,
                unit_kind: 0x2E64_8431,
                bundles: [0x3A82_63E2, 0x6827_D126, 0xC3EC_8B4D, 0x9957_A6A5],
            },
            Self::W3 => RetailProfile {
                jobs: [1, 2, 2],
                shoppers: 7,
                goods: 15,
                slots: [0, 2, 1],
                shifts: [0, 0, 4, 4, 4, 3, 0, 0],
                population: 10.0,
                labor_tuning: 6.0,
                profit: 80.0,
                vacancy_tuning: 68.0,
                category: 0x8333_23DF,
                unit_kind: 0x03DD_CA70,
                bundles: [0x3A82_63E3, 0x6827_D127, 0xC3EC_8B4C, 0xCA24_5C9C],
            },
        }
    }

    pub(super) fn bundles(self) -> impl Iterator<Item = u32> {
        COMMON.into_iter().chain(self.profile().bundles)
    }

    pub(super) fn excluded(self) -> impl Iterator<Item = u32> {
        OTHER_PROVIDERS.iter().copied().chain(
            [Self::W1, Self::W2, Self::W3]
                .into_iter()
                .filter(move |other| *other != self)
                .flat_map(|other| other.profile().bundles),
        )
    }
}
