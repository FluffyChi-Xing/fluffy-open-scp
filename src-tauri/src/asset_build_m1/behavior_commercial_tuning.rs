//! Authored per-wealth inventory and build-time checkout happiness policy.
//! Stock 2D0AB109/10A/10B specialize nearby HappinessToken output through
//! 0E854247 (op 8, data16=1, data8=1). No rule or resident return is replaced.
pub(super) const CHECKOUT_HAPPINESS: (u32, f32) = (0x0E85_4247, 1.0);
pub(super) const GOODS_INPUTS: [&str; 3] = ["goodsW1", "goodsW2", "goodsW3"];

pub(super) fn goods(
    params: &serde_json::Value,
    active: [bool; 3],
    fallback: i32,
) -> Result<i32, String> {
    let read = |field: &str, default| {
        if params[field].is_null() {
            return Ok(default);
        }
        params[field]
            .as_i64()
            .filter(|value| (1..=100_000).contains(value))
            .map(|value| value as i32)
            .ok_or_else(|| format!("COMMERCIAL_PARAMETER: {field}"))
    };
    let common = if GOODS_INPUTS.iter().all(|field| !params[*field].is_null()) {
        fallback
    } else {
        read("goods", fallback)?
    };
    // The city has substantially more W1 shoppers. Keep the former common
    // quantity as the W2 baseline; explicit tier quantities always win.
    let tiers = [
        read(GOODS_INPUTS[0], (common * 2).min(100_000))?,
        read(GOODS_INPUTS[1], common)?,
        read(GOODS_INPUTS[2], (common / 2).max(1))?,
    ];
    // The mixed selection is subsequently split by behavior_units into one
    // native street provider per Unit. Each final selection reads its tier.
    Ok(
        if active.into_iter().filter(|enabled| *enabled).count() == 1 {
            tiers[active.into_iter().position(|enabled| enabled).unwrap()]
        } else {
            common
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inventory_uses_the_selected_tier_and_preserves_legacy_fallbacks() {
        let params = json!({"goods": 450, "goodsW1": 120, "goodsW3": 360});
        for (index, expected) in [120, 450, 360].into_iter().enumerate() {
            assert_eq!(
                goods(&params, std::array::from_fn(|tier| tier == index), 750).unwrap(),
                expected
            );
        }
        assert_eq!(goods(&json!({}), [false, true, false], 750).unwrap(), 750);
        for (index, expected) in [900, 450, 225].into_iter().enumerate() {
            assert_eq!(
                goods(
                    &json!({"goods":450}),
                    std::array::from_fn(|tier| tier == index),
                    750
                )
                .unwrap(),
                expected
            );
        }
        assert_eq!(
            goods(&json!({"goods":100000}), [true, false, false], 750).unwrap(),
            100000
        );
        assert_eq!(
            goods(&json!({"goods":1}), [false, false, true], 750).unwrap(),
            1
        );
        assert_eq!(
            goods(
                &json!({"goods":0,"goodsW1":1,"goodsW2":2,"goodsW3":3}),
                [false, true, false],
                750
            )
            .unwrap(),
            2
        );
        for invalid in [json!(0), json!(-1), json!(1.5), json!(100001), json!("30")] {
            assert!(goods(&json!({"goodsW2": invalid}), [false, true, false], 750).is_err());
        }
    }

    #[test]
    fn mixed_compilation_writes_each_inventory_and_requires_checkout_happiness() {
        use super::super::{behavior_commercial_mall as mall, behavior_units, property_edit};
        use sc_properties::{PropType, PropertyFile, Value};
        let program = behavior_units::compile(
            None,
            &[json!({
                "id":"retail", "type":"commercial", "unit":"retail",
                "params":{"commercialProfile":"ground-mall-mixed-v1",
                    "shoppersW1":20,"shoppersW2":30,"shoppersW3":40,
                    "goodsW1":789,"goodsW2":456,"goodsW3":123}
            })],
        )
        .unwrap();
        assert_eq!(program.units.len(), 3);
        for (index, (scope, expected)) in program.units.iter().zip([789.0, 456.0, 123.0]).enumerate() {
            let recipe = scope.program.commercial.as_ref().unwrap();
            let mut unit = PropertyFile::default();
            property_edit::set_scalar(
                &mut unit,
                mall::INCOME_CAPACITY,
                PropType::Float,
                Value::Float(0.0),
            );
            let bytes = recipe
                .patch(&property_edit::encode(&unit).unwrap())
                .unwrap();
            let mut written = PropertyFile::parse(&bytes).unwrap();
            assert_eq!(
                written.get(0x0DA3_EE15).unwrap().scalar(),
                Some(&Value::Float(expected))
            );
            assert_eq!(
                written.get(CHECKOUT_HAPPINESS.0).unwrap().scalar(),
                Some(&Value::Float(1.0))
            );
            assert_eq!(recipe.floor_income_input(), mall::FLOOR_INCOME_INPUTS[index]);
            assert!(recipe.bundles().contains(&mall::INTERNAL[index]));
            assert!(recipe.bundles().contains(&mall::CUSTOMER_STATE[index]));
            assert!(recipe.bundles().contains(&mall::COMMERCIAL_CLASS[index]));
            assert_eq!(written.get(mall::RETURN_INTERVAL).unwrap().scalar(),
                       Some(&Value::Vector2(mall::RETURN_INTERVAL_VALUE)));
            assert_eq!(written.get(mall::SALES_MULTIPLIER).unwrap().scalar(),
                       Some(&Value::Float(mall::SALES_MULTIPLIERS[index])));
            written
                .values
                .retain(|property| property.hash != CHECKOUT_HAPPINESS.0);
            assert!(recipe.verify(&written).is_err());
        }
    }
}
