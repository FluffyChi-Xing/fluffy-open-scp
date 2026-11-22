//! Optional native landmark accounting required by daily-profit panels.
//! Native landmark template 48E0C000:4E01E311 enables C14044C4 in A4.
use sc_properties::{Key, Kind, PropType, Property, PropertyEncoding, PropertyFile, Value};

pub(crate) const PROVIDER: u32 = 0xC140_44C4;
pub(crate) const DAILY_BINS: [u32; 4] = [2780020242, 3459385487, 1038792412, 2430600245];
const ENABLE: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
fn array(file: &PropertyFile, hash: u32) -> &[Value] {
    match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Array(v)) => v,
        _ => &[],
    }
}
fn has(values: &[Value], id: u32) -> bool {
    values
        .iter()
        .any(|v| matches!(v, Value::Key(k) if k.instance == id))
}
pub(crate) fn ensure(
    unit: &mut PropertyFile,
    effective: &PropertyFile,
    native: &PropertyFile,
) -> Result<bool, String> {
    for bin in DAILY_BINS {
        if !has(array(native, 0x07E0_AD23), bin) {
            return Err(format!("PANEL_PROFIT_LEDGER_BIN:{bin:08X}"));
        }
    }
    if array(native, 0x0808_0D58).is_empty() {
        return Err("PANEL_PROFIT_LEDGER_RULES".into());
    }
    if [PROVIDER, 0x3B8E_EF5B].into_iter().any(|provider| {
        !has(array(effective, 0x0A02_09B2), provider)
            && ENABLE
                .iter()
                .any(|row| has(array(effective, *row), provider))
    }) {
        return Ok(false);
    }
    if has(array(effective, 0x0A02_09B2), PROVIDER) {
        return Err("PANEL_PROFIT_LEDGER_EXCLUDED".into());
    }
    let mut values = array(effective, 0x0A02_09A4).to_vec();
    values.push(Value::Key(Key {
        type_id: 0,
        group: 0,
        instance: PROVIDER,
    }));
    let property = Property {
        hash: 0x0A02_09A4,
        prop_type: PropType::Key,
        kind: Kind::Array(values),
        encoding: PropertyEncoding {
            flags: 0x809C,
            array_item_size: Some(12),
        },
    };
    if let Some(old) = unit.values.iter_mut().find(|p| p.hash == property.hash) {
        *old = property;
    } else {
        unit.values.push(property);
    }
    if !has(array(unit, 0x0A02_09A4), PROVIDER) {
        return Err("PANEL_PROFIT_LEDGER_BINDING".into());
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn field(hash: u32, ids: &[u32]) -> Property {
        Property {
            hash,
            prop_type: PropType::Key,
            kind: Kind::Array(
                ids.iter()
                    .map(|id| {
                        Value::Key(Key {
                            type_id: 0,
                            group: 0,
                            instance: *id,
                        })
                    })
                    .collect(),
            ),
            encoding: PropertyEncoding {
                flags: 0x809C,
                array_item_size: Some(12),
            },
        }
    }
    #[test]
    fn accounting_is_idempotent_and_requires_native_bins_and_rollover_rules() {
        let mut native = PropertyFile::default();
        native.values = vec![field(0x07E0_AD23, &DAILY_BINS), field(0x0808_0D58, &[1])];
        let mut unit = PropertyFile::default();
        let mut effective = PropertyFile::default();
        effective.values = vec![field(0x0A02_09A4, &[42])];
        assert!(ensure(&mut unit, &effective, &native).unwrap());
        assert!(has(array(&unit, 0x0A02_09A4), 42));
        let effective = unit.clone();
        assert!(!ensure(&mut unit, &effective, &native).unwrap());
        native.values.pop();
        assert!(
            ensure(&mut unit, &effective, &native)
                .unwrap_err()
                .contains("RULES")
        );
    }
    #[test]
    fn an_explicit_exclusion_is_not_silently_overridden() {
        let mut native = PropertyFile::default();
        native.values = vec![field(0x07E0_AD23, &DAILY_BINS), field(0x0808_0D58, &[1])];
        let mut effective = PropertyFile::default();
        effective.values = vec![field(0x0A02_09B2, &[PROVIDER])];
        assert_eq!(
            ensure(&mut PropertyFile::default(), &effective, &native).unwrap_err(),
            "PANEL_PROFIT_LEDGER_EXCLUDED"
        );
    }
}
