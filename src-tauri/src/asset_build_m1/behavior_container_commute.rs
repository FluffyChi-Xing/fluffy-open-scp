//! The visible parking slots read the commuter-car bins of their own Unit.
//! Keep the building's authored base staffing on that same Unit when it owns
//! an authored driveway and parking props. Business leaves remain connected.
use super::{behavior_nodes::ResolvedBehavior, behavior_units::BUILDING_STAFF, scene::{SceneDocument, SceneUnit}};

pub(super) fn route_base_staffing(program: &mut ResolvedBehavior, scene: Option<&SceneDocument>) {
    if !super::vehicle_paths::authored(scene) || !scene.is_some_and(|scene| {
        scene.units.iter().any(|unit| matches!(unit, SceneUnit::Prop {
            bin: 0, resource_id: Some(0x0AB4_DFE1..=0x0AB4_DFE3), ..
        }))
    }) {
        return;
    }
    // The Copan plop route already owns its base payroll on the host, so it
    // has no BUILDING_STAFF child to move. It still needs the ordinary ground
    // driveway binding and count-based parking draw.
    if program.host_base_labor {
        program.host_parking_staff = true;
        return;
    }
    let Some(index) = program.units.iter().position(|unit| unit.id == BUILDING_STAFF) else {
        return;
    };
    let staffing = program.units.remove(index).program;
    program.host_parking_staff = true;
    program.ensure_internal_path_ref |= staffing.ensure_internal_path_ref;
    program.set_float_fields.extend(staffing.set_float_fields);
    program.set_int_fields.extend(staffing.set_int_fields);
    for (target, source) in [
        (&mut program.ensure_bundles, staffing.ensure_bundles),
        (&mut program.remove_bundles, staffing.remove_bundles),
        (&mut program.remove_excluded_bundles, staffing.remove_excluded_bundles),
        (&mut program.declared_unit_columns, staffing.declared_unit_columns),
    ] {
        for value in source {
            if !target.contains(&value) { target.push(value); }
        }
    }
    program.remove_bundles.retain(|bundle| !program.ensure_bundles.contains(bundle));
    program.remove_fields.retain(|field| !program.set_float_fields.contains_key(field) && !program.set_int_fields.contains_key(field));
    for bundle in [0xD07C_5A04, 0xF236_BB93] {
        if !program.remove_excluded_bundles.contains(&bundle) {
            program.remove_excluded_bundles.push(bundle);
        }
    }
    program.notes.push("authored parking and base staffing share the host commuter bins".into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::{behavior_nodes, scene::ScenePath};
    use serde_json::json;

    #[test]
    fn copan_host_payroll_also_enables_the_final_driveway_and_parking_contract() {
        let labor = behavior_nodes::parse_labor(Some(&json!({"w1":200,"w2":100,"w3":20,"minWorkers":5}))).unwrap();
        let mut program = super::super::behavior_copan_base::compile_building(labor, &[
            json!({"id":"park","type":"park","params":{}}),
        ]).unwrap();
        assert!(program.host_base_labor);
        assert!(!program.units.iter().any(|unit| unit.id == BUILDING_STAFF));
        let before = program.clone();
        let scene = SceneDocument { units: vec![SceneUnit::Prop {
            bin: 0, resource_id: Some(0x0AB4_DFE1), transform: None, slot: None,
            scale: None, randomize_slot: None, percent_fill: None,
        }], paths: vec![ScenePath {kind: Some("vehicle".into()), entry: None,
            point_ids: Vec::new(), scope_local: true, road_connect: false}], ..Default::default() };
        route_base_staffing(&mut program, Some(&scene));
        assert!(program.host_parking_staff);
        assert_eq!(program.units, before.units);
        assert_eq!(program.set_float_fields, before.set_float_fields);
        assert_eq!(program.ensure_bundles, before.ensure_bundles);
    }

    #[test]
    fn parking_moves_the_authored_low_threshold_payroll_without_changing_business_jobs() {
        let labor = behavior_nodes::parse_labor(Some(&json!({"w1":300,"w2":0,"w3":0,"minWorkers":5}))).unwrap();
        let mut program = behavior_nodes::compile(labor, &[
            json!({"id":"office","type":"office","unit":"office","params":{"w1":80,"w2":10,"w3":0,"minWorkers":4}}),
        ]).unwrap();
        let baseline = program.clone();
        let mut scene = SceneDocument { units: vec![SceneUnit::Prop {
            bin: 0, resource_id: Some(0x0AB4_DFE1), transform: None, slot: None,
            scale: None, randomize_slot: None, percent_fill: None,
        }], ..Default::default() };
        route_base_staffing(&mut program, Some(&scene));
        assert_eq!(program, baseline);
        scene.paths.push(ScenePath {kind: Some("vehicle".into()), entry: None,
            point_ids: Vec::new(), scope_local: true, road_connect: false});
        route_base_staffing(&mut program, Some(&scene));
        assert!(program.host_parking_staff);
        assert_eq!(program.units, baseline.units[..1]);
        assert_eq!(program.set_float_fields[&0x0A79_17C5], 300.0);
        assert_eq!(program.set_float_fields[&0x0A79_17C8], 5.0);
        for bundle in [0x3064_5625, 0xD07C_5A04, 0xF236_BB93] {
            assert!(program.remove_excluded_bundles.contains(&bundle));
            assert!(!program.remove_bundles.contains(&bundle));
        }
    }
}
