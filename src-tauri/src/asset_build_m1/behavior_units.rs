//! Independent simulation scopes. Legacy unscoped recipes never enter here.
//! Each scope is compiled by the existing single-Unit compiler, with its own
//! parameters, bins and sinks. Sharing a scope still requires normal validation.
//! Explicit legacy scopes and host-owned workbench components share the binder.
use super::behavior_nodes::{
    LaborSpec, ResolvedBehavior, compile_with_profile, parse_labor, parse_nodes,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

pub(super) const BUILDING_STAFF: &str = "building-staff";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct UnitBehavior {
    pub id: String,
    pub program: ResolvedBehavior,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub shares_building_site: bool,
}

pub(super) fn scoped(nodes: &[Value]) -> bool {
    nodes.iter().any(|node| !node["unit"].is_null())
}

pub(super) fn compile(
    labor: Option<LaborSpec>,
    nodes: &[Value],
) -> Result<ResolvedBehavior, String> {
    compile_scopes(labor, nodes, false)
}

pub(super) fn compile_workbench(
    labor: Option<LaborSpec>,
    nodes: &[Value],
    author_tourism: bool,
) -> Result<ResolvedBehavior, String> {
    let primary = ["commercial", "office", "hotel", "residence", "education", "tourism", "park", "landvalue", "tree"]
        .into_iter()
        .find(|&kind| nodes.iter().any(|node| node["type"] == kind))
        .ok_or("BEHAVIOR_UNIT_SCOPE: no primary node")?;
    let (host_nodes, mut components): (Vec<_>, Vec<_>) = nodes.iter().cloned().partition(|node| {
        node["type"] == "utilities-core" || node["type"] == primary
            || (primary == "commercial" && node["type"] == "office")
    });
    let native_tourism_host = author_tourism && super::author_tourism::selected(&host_nodes);
    let mut host = super::behavior_mega_tower::compile_host(labor, host_nodes, native_tourism_host)?;
    if !components.is_empty() {
        components.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
        for node in &mut components {
            node["unit"] = node["id"].clone();
        }
        host.units = compile_scopes(None, &components, true)?.units;
    }
    Ok(host)
}

fn compile_scopes(
    labor: Option<LaborSpec>,
    nodes: &[Value],
    shares_building_site: bool,
) -> Result<ResolvedBehavior, String> {
    compile_scopes_mode(labor,nodes,shares_building_site,true)
}

pub(super) fn compile_copan(labor: Option<LaborSpec>,nodes: &[Value]) -> Result<ResolvedBehavior,String> {
    compile_scopes_mode(labor,nodes,true,false)
}

fn compile_scopes_mode(
    labor: Option<LaborSpec>,
    nodes: &[Value],
    shares_building_site: bool,
    allow_complete_tourism_donor: bool,
) -> Result<ResolvedBehavior,String> {
    let nodes = parse_nodes(nodes)?;
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    let mut utilities = Vec::new();
    for mut node in nodes {
        if node["type"] == "utilities-core" {
            if !node["unit"].is_null() {
                return Err("BEHAVIOR_UNIT_SCOPE: utilities belong to host".into());
            }
            utilities.push(node);
            continue;
        }
        let scope = node["unit"]
            .as_str()
            .map(str::trim)
            .filter(|id| {
                !id.is_empty() && id.len() <= 128 && ![super::behavior_container::SCOPE, BUILDING_STAFF].contains(id)
            })
            .ok_or("BEHAVIOR_UNIT_SCOPE: each leaf requires a unit")?
            .to_owned();
        node.as_object_mut().unwrap().remove("unit");
        if let Some((_, group)) = groups.iter_mut().find(|(id, _)| *id == scope) {
            group.push(node);
        } else {
            groups.push((scope, vec![node]));
        }
    }
    let building_staff = labor.filter(|staff| [staff.w1, staff.w2, staff.w3] != [0; 3]);
    if groups.is_empty() || groups.len() + usize::from(building_staff.is_some()) > 16 {
        return Err("BEHAVIOR_UNIT_SCOPE: expected 1..16 units".into());
    }
    let mut compiled = Vec::new();
    for (id, group) in groups {
        let own_labor = group
            .iter()
            .filter_map(|node| node.get("labor"))
            .collect::<Vec<_>>();
        if own_labor.len() > 1 {
            return Err("BEHAVIOR_UNIT_SCOPE: declare labor once per unit".into());
        }
        let own_labor = if let Some(value) = own_labor.first() {
            parse_labor(Some(value))?
        } else {
            native_labor(&group)
        };
        let author = allow_complete_tourism_donor && super::author_tourism::selected(&group);
        if let [leaf] = group.as_slice()
            && let Some(recipe) = super::behavior_commercial::select(&group, own_labor)?
            && let Some(tiers) = recipe.mixed_tiers()
        {
            // A stock sink tests OpenShoppers on its own Unit. Give each
            // wealth tier its own budget while retaining the stock sink IDs
            // that resident and regional dispatch already search for.
            let staffing = super::behavior_commercial_mixed::split_staffing(tiers, recipe.labor(own_labor)?);
            for (part, (index, quota)) in tiers.into_iter().enumerate()
                .filter(|(_, quota)| *quota > 0).enumerate()
            {
                let mut leaf = leaf.clone();
                let scope = if part == 0 { id.clone() } else { format!("retail-wealth-{}:{id}", index + 1) };
                for (tier, field) in super::behavior_commercial_mixed::SHOPPER_INPUTS.into_iter().enumerate() {
                    leaf["params"][field] = serde_json::json!(if tier == index { quota } else { 0 });
                }
                for field in ["jobsW1", "jobsW2", "jobsW3", "retailMinWorkers"] {
                    leaf["params"].as_object_mut().unwrap().remove(field);
                }
                let mut program = compile_with_profile(Some(staffing[index]), &[leaf], false)?;
                program.unit_scope = Some(scope.clone());
                compiled.push(UnitBehavior { id: scope, program, shares_building_site });
            }
        } else {
            let mut program = if !allow_complete_tourism_donor
                && matches!(group.as_slice(),[leaf] if leaf["type"]=="park") {
                super::behavior_park_core::compile(&group[0]["params"])?
            } else {compile_with_profile(own_labor, &group, author)?};
            program.unit_scope = Some(id.clone());
            compiled.push(UnitBehavior { id, program, shares_building_site });
        }
    }
    let mut primary = compile_with_profile(None, &utilities, false)?;
    primary.unit_scope = Some(super::behavior_container::SCOPE.into());
    let garbage = super::behavior_container::GARBAGE;
    if primary.ensure_bundles.contains(&garbage) && !primary.remove_excluded_bundles.contains(&garbage) {
        primary.remove_excluded_bundles.push(garbage);
    }
    if primary.ensure_bundles.contains(&garbage) {
        let schedule = super::behavior_container::GARBAGE_SCHEDULE;
        if !primary.remove_excluded_bundles.contains(&schedule) {
            primary.remove_excluded_bundles.push(schedule);
        }
    }
    if let Some(staff) = building_staff {
        let jobs = super::behavior_nodes::add_job_counts(staff, [0; 3])?;
        if staff.min_workers > jobs.iter().sum() {
            return Err("BEHAVIOR_UNIT_SCOPE: building staffing threshold exceeds jobs".into());
        }
        let mut staffing = compile_with_profile(Some(staff), &[], false)?;
        staffing.unit_scope = Some(BUILDING_STAFF.into());
        let toggle = 0x44F5_CF6C;
        staffing.ensure_bundles.push(toggle);
        staffing.remove_excluded_bundles.push(toggle);
        staffing.remove_bundles.retain(|&id| id != toggle);
        compiled.push(UnitBehavior { id: BUILDING_STAFF.into(), program: staffing, shares_building_site });
    }
    primary.notes.push(
        "managed-building-container/v7 (native host lifecycle, modular bars and host garbage service)".into(),
    );
    if compiled.len() > 16 || compiled.iter().map(|unit| &unit.id).collect::<HashSet<_>>().len() != compiled.len() {
        return Err("BEHAVIOR_UNIT_SCOPE: expanded components require at most 16 unique unit IDs".into());
    }
    primary.units = compiled;
    Ok(primary)
}

// Native independent retail staffing: ground D1 stores and EP1 W2 mall.
// These are only defaults for a new isolated unit; legacy labor is untouched.
fn native_labor(nodes: &[Value]) -> Option<LaborSpec> {
    let node = nodes.iter().find(|node| node["type"] == "commercial")?;
    if super::behavior_commercial::has_job_inputs(&node["params"]) {
        return None;
    }
    let (jobs, minimum) = match node["params"]["commercialProfile"].as_str() {
        Some("ground-commercial-w1-v1") => ([5, 2, 0], 3),
        Some("ground-commercial-w3-v1") => ([1, 2, 2], 3),
        Some("ground-mall-mixed-v1" | "ground-megatower-mall-w2-v1") => ([120, 100, 30], 20),
        _ => ([3, 2, 1], 3),
    };
    Some(LaborSpec {
        w1: jobs[0],
        w2: jobs[1],
        w3: jobs[2],
        min_workers: minimum,
        schedule: None,
        slots: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn adding_an_incompatible_node_keeps_commercial_and_office_on_the_host() {
        let labor = parse_labor(Some(&json!({"w1":200,"w2":100,"w3":20,"minWorkers":5}))).unwrap();
        let base = vec![
            json!({"id":"office","type":"office","params":{}}),
            json!({"id":"store","type":"commercial","params":{"commercialProfile":"ground-mall-mixed-v1"}}),
        ];
        let expected = super::super::behavior_mega_tower::compile_building(labor, &base, false).unwrap();
        let mut nodes = base;
        nodes.extend([
            json!({"id":"school","type":"education","params":{}}),
            json!({"id":"park","type":"park","params":{}}),
        ]);
        let mut resolved = super::super::behavior_mega_tower::compile_building(labor, &nodes, false).unwrap();
        nodes.reverse();
        assert_eq!(resolved, super::super::behavior_mega_tower::compile_building(labor, &nodes, false).unwrap());
        let components = std::mem::take(&mut resolved.units);
        assert_eq!(resolved, expected);
        assert_eq!(components.iter().map(|unit| unit.id.as_str()).collect::<Vec<_>>(), ["park", "school"]);
        assert!(components.iter().all(|unit| unit.shares_building_site));
    }

    #[test]
    fn building_staff_and_business_jobs_are_additive_in_both_routes() {
        let labor = parse_labor(Some(&json!({"w1":0,"w2":20,"w3":10,"minWorkers":5}))).unwrap();
        let office = json!({"id":"office","type":"office","params":{"w1":40,"w2":30,"w3":20}});
        let retail = json!({"id":"retail","type":"commercial","params":{"commercialProfile":"ground-mall-mixed-v1","jobsW1":10,"jobsW2":5,"jobsW3":0}});
        let jobs = |program: &ResolvedBehavior| {
            [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7].map(|field| program.set_float_fields[&field])
        };
        assert_eq!(jobs(&compile_with_profile(labor, &[office.clone()], false).unwrap()), [40.0, 50.0, 30.0]);
        assert_eq!(jobs(&compile_with_profile(labor, &[retail.clone()], false).unwrap()), [10.0, 25.0, 10.0]);

        let mut office = office;
        office["unit"] = json!("office");
        let mut retail = retail;
        retail["unit"] = json!("retail");
        let container = compile(labor, &[office, retail]).unwrap();
        let unit_jobs: Vec<_> = container.units.iter().map(|unit| jobs(&unit.program)).collect();
        assert_eq!(unit_jobs, [
            [40.0, 30.0, 20.0], [3.0, 1.0, 0.0], [3.0, 2.0, 0.0],
            [4.0, 2.0, 0.0], [0.0, 20.0, 10.0],
        ]);
        assert_eq!(container.units[4].program.set_float_fields[&0x0A79_17C3], 30.0);
        assert!(container.set_float_fields.get(&0x0A79_17C5).is_none());
    }

    #[test]
    fn isolates_office_and_tourism_parameters_and_keeps_legacy_validation() {
        let nodes = [
            json!({"id":"office", "type":"office", "unit":"work", "params":{}}),
            json!({"id":"visitors", "type":"tourism", "unit":"visits", "params":{"tourismProfile":"native-tourism-v1"}}),
        ];
        let program = compile(None, &nodes).unwrap();
        assert!(program.office.is_none());
        assert!(program.native_tourism.is_none());
        assert_eq!(program.units.len(), 2);
        assert!(program.units[0].program.office.is_some());
        assert!(program.units[1].program.office.is_none());
        assert!(program.units[1].program.native_tourism.is_some());
        assert_eq!(program.units[1].program.set_float_fields[&0x0C0F_228C], 6.0);
        assert!(!program.set_float_fields.contains_key(&0x0C0F_228C));
        assert_eq!(compile(None, &nodes).unwrap().units[1].id, "visits");
    }

    #[test]
    fn conflicts_remain_inside_a_unit_and_missing_scopes_fail() {
        let nodes = [
            json!({"id":"a","type":"office","unit":"shared","params":{}}),
            json!({"id":"b","type":"residence","unit":"shared","params":{"residenceProfile":"ground-residence-w1-v1"}}),
        ];
        assert!(compile(None, &nodes).is_err());
        let mut separate = nodes.clone();
        separate[1]["unit"] = json!("home");
        assert!(compile(None, &separate).is_ok());
        separate[1].as_object_mut().unwrap().remove("unit");
        assert!(compile(None, &separate).is_err());
    }

    #[test]
    fn tower_business_types_compile_in_six_separate_scopes() {
        let nodes = vec![
            json!({"id":"office","type":"office","unit":"office","params":{}}),
            json!({"id":"visits","type":"tourism","unit":"visits","params":{"tourismProfile":"native-tourism-v1"}}),
            json!({"id":"mall","type":"commercial","unit":"mall","params":{"commercialProfile":"ground-mall-mixed-v1"}}),
            json!({"id":"home","type":"residence","unit":"home","params":{"residenceProfile":"ground-residence-w1-v1"}}),
            json!({"id":"park","type":"park","unit":"park","params":{}}),
            json!({"id":"school","type":"education","unit":"school","params":{}}),
        ];
        let program = compile(None, &nodes).unwrap();
        assert_eq!(program.units.len(), 8);
        let malls: Vec<_> = program.units.iter().filter(|unit| unit.program.commercial.is_some()).collect();
        assert_eq!(malls.len(), 3);
        for (index, mall) in malls.iter().enumerate() {
            let recipe = mall.program.commercial.as_ref().unwrap();
            let admission = super::super::behavior_commercial_mixed::STREET;
            assert_eq!(
                recipe.bundles().into_iter().filter(|id| admission.contains(id)).collect::<Vec<_>>(),
                [admission[index]],
            );
        }
        for (hash, total) in [(0x0A79_17C5, 120.0), (0x0A79_17C6, 100.0), (0x0A79_17C7, 30.0)] {
            assert_eq!(malls.iter().map(|unit| unit.program.set_float_fields[&hash]).sum::<f64>(), total);
        }
        let unscoped = nodes
            .iter()
            .map(|node| {
                let mut node = node.clone();
                node.as_object_mut().unwrap().remove("unit");
                node
            })
            .collect::<Vec<_>>();
        assert!(compile_with_profile(None, &unscoped, false).is_err());
    }
}
