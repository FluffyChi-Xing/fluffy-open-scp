//! Copan's business-free plop layer. See the hash-pinned extraction audit in
//! tmp/residence-minimal-loop-20261125/copan-basic-plop-audit.json.
//! Native column selection/encoding: Packager independent_park_profile.flat,
//! build_candidate.py:39-42. No tower exclusions or concrete Copan Asset.
use super::{behavior_nodes::{LaborSpec, ResolvedBehavior}, behavior_templates::{ENABLE, EXCLUDED, array, contains},
    property_edit::{encode, set_scalar}};
use sc_properties::{PropType, PropertyFile, Value};
use sha2::{Digest, Sha256};

pub(super) const VERSION: &str = "copan-plop-base-v9";
const SOURCE: &[u8] = include_bytes!("../../resources/sc5/copan-plop-base.bin");
const SOURCE_SHA: &str = "8a9f05319dc140dbbf4a2313ab638a5d3341c24b7cdb95940ee15da4364fc5b8";
const REQUIRED: &[u32] = &[0x4893_9D96, 0x825B_4CA4, 0xD7C6_2954, 0x54AB_8B16,
    0x1B94_6073, 0x44F5_CF6C, 0x3064_5625, 0x406C_CC43];
const FORBIDDEN: &[u32] = &[0x182D_2924, 0x933F_0FBF, 0xAB9A_0A3E, 0xD4C3_0C02,
    0xA8CB_9DE1, 0xAC8B_8A48, 0x999F_8484, 0x999F_8485, 0x78A7_30CD,
    0xEBFC_1753, 0xE153_69A2, 0x75D2_2FD0, 0x75D2_2FD1, 0x75D2_2FD2, 0x8A60_B969];
const JOBS: [u32; 5] = [0x0A79_17C3, 0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7, 0x0A79_17C8];
const BUSINESS_INPUTS: &[u32] = &[0x0A79_17CB, 0x0E33_502E, 0x0E33_5022, 0x0A79_17CC,
    0x0E54_2F39, 0x0D8C_29F6, 0x0D8C_29FE, 0x0D8C_2A01, 0x07E0_AD09, 0x0AFE_27CA,
    0x0E51_C90F, 0x0EC5_D62E, 0x0DF1_467C, 0x0BF5_D2E3, 0x0BF5_D2E4];

pub(super) fn source() -> Result<PropertyFile, String> {
    if format!("{:x}", Sha256::digest(SOURCE)) != SOURCE_SHA {
        return Err("BEHAVIOR_COPAN_BASE: source hash".into());
    }
    PropertyFile::parse(SOURCE).map_err(|e| format!("BEHAVIOR_COPAN_BASE: {e}"))
}
fn active(unit: &PropertyFile, id: u32) -> bool {
    contains(unit, &ENABLE, id) && !contains(unit, &[EXCLUDED], id)
}

pub(super) fn standalone(nodes: &[serde_json::Value]) -> bool {
    !nodes.is_empty() && nodes.iter().all(|node| node["type"] == "utilities-core")
}

pub(super) fn supports(nodes: &[serde_json::Value]) -> bool {
    standalone(nodes) || (nodes.iter().any(|n| matches!(n["type"].as_str(),
        Some("residence" | "commercial" | "tourism" | "office" | "park")))
        && nodes.iter().all(|n| matches!(n["type"].as_str(),
            Some("utilities-core" | "residence" | "commercial" | "tourism" | "office" | "park" | "tree" | "landvalue"))))
}

pub(super) fn compile_building(labor: Option<LaborSpec>, nodes: &[serde_json::Value]) -> Result<ResolvedBehavior, String> {
    if standalone(nodes) { return compile(labor,nodes); }
    super::behavior_policy::needs_components(nodes)?;
    let mut nodes=super::behavior_nodes::parse_nodes(nodes)?;
    nodes.sort_by(|a,b|a["id"].as_str().cmp(&b["id"].as_str()));
    for node in &mut nodes {
        if node["type"]=="utilities-core" { continue; }
        node["unit"]=node["id"].clone();
        if !node["params"].is_object() { node["params"]=serde_json::json!({}); }
        let default=match node["type"].as_str() {
            Some("residence")=>Some(("residenceProfile","ground-residence-w1-v1")),
            Some("commercial")=>None,
            Some("tourism")=>Some(("tourismProfile","native-tourism-v1")),
            _=>None,
        };
        if let Some((field,value))=default && node["params"][field].is_null() {
            node["params"][field]=serde_json::json!(value);
        }
        if node["type"]=="commercial" { commercial_plop_params(&mut node["params"])?; }
        if node["type"]=="residence" { super::behavior_copan_residence::normalize(&mut node["params"]); }
    }
    let components=super::behavior_units::compile_copan(None,&nodes)?;
    let utilities:Vec<_>=nodes.iter().filter(|n|n["type"]=="utilities-core").cloned().collect();
    // Facility employment is a base service. Business-specific jobs remain
    // with their business; do not create an independent staffing facility.
    let mut program=compile(labor,&utilities)?;
    program.units=components.units;
    for scope in &mut program.units {
        clear_legacy_parent(&mut scope.program);
        super::behavior_tourism_crown::configure(&mut scope.program)?;
        // Copan 40E0C000:4DC31F9B explicitly excludes 30645625; its
        // residential state provider replaces the facility lifecycle. Keep
        // 30645625 on the visible host, not on the private residence as well.
        if scope.program.templates.residence() {
            super::behavior_copan_residence::configure(&mut scope.program)?;
            if !scope.program.remove_bundles.contains(&0x3064_5625) {
                scope.program.remove_bundles.push(0x3064_5625);
            }
        }
    }
    super::behavior_copan_services::allocate(&mut program)?;
    protect_native_base(&mut program)?;
    program.copan_plop_base=true;
    program.notes.push(VERSION.into());
    Ok(program)
}

// Ground RCI retail's 6827D125/126/127 profit cycles can abandon a unit.
// Ploppable retail instead owns the native EP1 mall sales/income cycle.
// Translate older editor selections here; never silently fall back to RCI.
fn commercial_plop_params(params: &mut serde_json::Value) -> Result<(), String> {
    use super::{behavior_commercial_mall as mall, behavior_commercial_mixed as mixed};
    let profile = super::behavior_commercial::profile_name(params)?;
    let tier=match profile {
        "ground-commercial-w2-v1" | mall::PROFILE => {
            params["commercialProfile"]=mall::PROFILE.into();
            return Ok(());
        }
        mixed::PROFILE => {
            params["commercialProfile"] = mixed::PROFILE.into();
            return Ok(());
        }
        "ground-commercial-w1-v1" => 0,
        "ground-commercial-w3-v1" => 2,
        _ => return Err("COMMERCIAL_PARAMETER: commercialProfile".into()),
    };
    let quota=params["shoppers"].clone();
    for (i,field) in mixed::SHOPPER_INPUTS.into_iter().enumerate() {
        params[field]=if i==tier {
            if quota.is_null() { serde_json::json!(if tier==0 {360}else{90}) } else {quota.clone()}
        } else {serde_json::json!(0)};
    }
    if params["goods"].is_null() { params["goods"]=serde_json::json!(if tier==0 {1500}else{375}); }
    params["commercialProfile"]=mixed::PROFILE.into();
    Ok(())
}

pub(super) fn verify_commercial(unit: &PropertyFile, program: &ResolvedBehavior) -> Result<(), String> {
    if let Some(commercial)=&program.commercial {
        if !commercial.uses_tower_income()
            || [0x6827_D125,0x6827_D126,0x6827_D127].into_iter().any(|id|active(unit,id)) {
            return Err("BEHAVIOR_COPAN_BASE: RCI commercial abandonment cycle".into());
        }
        for id in [0xDD6E_83EC,0xF51A_B185] {
            if !active(unit,id) { return Err(format!("BEHAVIOR_COPAN_BASE: mall cycle {id:08X}")); }
        }
    }
    Ok(())
}

pub(super) fn verify_component(unit: &PropertyFile) -> Result<(), String> {
    // Native 406CCC43's OnCreate EA7382FD sets IsToy (41). The RCI
    // residential abandonment root 246F5982 requires IsToy=0. Retain the
    // native creation provider, rather than manufacturing a resident count
    // or pre-setting a happiness/fire/utility flag to suppress that branch.
    if !active(unit,0x406C_CC43) {
        return Err("BEHAVIOR_COPAN_BASE: missing native plop creation provider".into());
    }
    if unit.get(0x0AFE_27B1)!=source()?.get(0x0AFE_27B1) {
        return Err("BEHAVIOR_COPAN_BASE: component must retain the native plop building type".into());
    }
    if unit.get(super::menu::HASH_PARENT).is_some() || unit.get(0x07E0_AD09).is_some()
        || array(unit,0x0A88_D179)?.iter().any(|v|matches!(v,Value::Int32(11|13|15))) {
        return Err("BEHAVIOR_COPAN_BASE: inherited child identity or failed initial state".into());
    }
    Ok(())
}

pub(super) fn patch_residence_component(unit: &mut PropertyFile) -> Result<(), String> {
    let mut excluded=array(unit,EXCLUDED)?.to_vec();
    let lifecycle=super::behavior_templates::key(0x3064_5625);
    if !excluded.contains(&lifecycle) {
        excluded.push(lifecycle);
        super::property_edit::set_array_with_flags(unit,EXCLUDED,PropType::Key,excluded,0x809C);
    }
    verify_residence_component(unit)
}

pub(super) fn verify_residence_component(unit: &PropertyFile) -> Result<(), String> {
    if active(unit,0x3064_5625) || !contains(unit,&[EXCLUDED],0x3064_5625)
        || !active(unit,0x933F_0FBF) || !active(unit,0x9DEF_44FB) {
        return Err("BEHAVIOR_COPAN_BASE: residence must use its native occupancy lifecycle, not the host facility lifecycle".into());
    }
    Ok(())
}

fn protect_native_base(program: &mut ResolvedBehavior) -> Result<(), String> {
    let base=source()?;
    // This path already owns the flattened native plop lifecycle. Legacy
    // labor's school-parent substitution would reintroduce unrelated state.
    clear_legacy_parent(program);
    program.remove_fields.retain(|h|base.get(*h).is_none());
    program.remove_bundles.retain(|id|!active(&base,*id));
    Ok(())
}

fn clear_legacy_parent(program: &mut ResolvedBehavior) {
    program.parent_template=None;
    program.behavior_category=None;
    program.declared_unit_columns.retain(|hash|![super::menu::HASH_PARENT,0x0AFE_27B1].contains(hash));
}

pub(super) fn validate_base_menu(
    resources: &super::connector::Resources,
    instance: u32,
    donor: Option<(&super::connector::Resources,u32)>,
    program: &ResolvedBehavior,
    panel: Option<&super::panel::PanelPlan>,
) -> Result<super::menu_contract::MenuContractReport,String> {
    use super::{behavior_container,behavior_container_ui,behavior_container_bars,menu};
    use dbpf::ResourceId;
    if !program.copan_plop_base || !program.units.is_empty()
        || program.unit_scope.as_deref()!=Some(behavior_container::SCOPE) {
        return Err("BEHAVIOR_COPAN_BASE: standalone base contract only".into());
    }
    let mut report=super::menu_contract::validate(resources,instance,menu::lot_group(),donor,true)?;
    let get=|group| resources.get(&ResourceId{type_id:menu::PROPERTY_TYPE,group,instance})
        .ok_or_else(||format!("BEHAVIOR_COPAN_BASE: missing resource {group:08X}:{instance:08X}"))
        .and_then(|bytes|PropertyFile::parse(bytes).map_err(|e|e.to_string()));
    let menu=get(menu::MENU_GROUP)?;
    for hash in [0x08F6_72C7,0x09E6_83AC,0x09E6_83AD,0x09E9_1B9C,0x0C7F_4336] {
        if menu.get(hash).is_none() {return Err(format!("BEHAVIOR_COPAN_BASE: missing panel field {hash:08X}"));}
    }
    let host=get(menu::UNIT_GROUP)?;
    verify(&host,Some(program))?;
    super::behavior_unit_build::verify(resources,instance,program)?;
    super::interior_lighting::verify_landmark_activation(&host)?;
    if let Some(panel)=panel {
        behavior_container_ui::verify_presentation(resources,instance,program,Some(panel))?;
    } else {behavior_container_ui::verify(resources,instance,program)?;}
    behavior_container_bars::verify(resources,instance,program)?;
    report.checks.push("Copan standalone base; native services, lifecycle, menu and bars verified; runtime acceptance pending".into());
    Ok(report)
}

pub(super) fn compile(labor: Option<LaborSpec>, nodes: &[serde_json::Value]) -> Result<ResolvedBehavior, String> {
    let mut program = super::behavior_nodes::compile_with_profile(labor, nodes, false)?;
    // Generic family cleanup is not the owner of native base columns/services.
    protect_native_base(&mut program)?;
    program.copan_plop_base = true;
    program.host_base_labor = labor.is_some_and(|l| l.w1 + l.w2 + l.w3 > 0);
    program.unit_scope = Some(super::behavior_container::SCOPE.into());
    program.notes.push(VERSION.into());
    Ok(program)
}

pub(super) fn patch(identity: &[u8]) -> Result<Vec<u8>, String> {
    let identity = PropertyFile::parse(identity).map_err(|e| e.to_string())?;
    let mut base = source()?;
    // Identity/presentation only. Utilities are the Copan base defaults until
    // the authored utilities/labor compiler applies its explicit inputs.
    for hash in [0x08E1_7ED7, 0x0ACF_FA64, 0x09FB_78CB, 0x0A8A_41DA, 0x09AE_19D7] {
        if let Some(property) = identity.get(hash) {
            base.values.retain(|p| p.hash != hash);
            base.values.push(property.clone());
        }
    }
    // Empty base has no payroll. This is the existing ordinary-container
    // zero-job contract; authored staffing subsequently replaces these inputs.
    for hash in JOBS { set_scalar(&mut base, hash, PropType::Float, Value::Float(0.0)); }
    verify(&base, None)?;
    encode(&base)
}

pub(super) fn verify(unit: &PropertyFile, program: Option<&ResolvedBehavior>) -> Result<(), String> {
    let native = source()?;
    if unit.get(0x0AFE_27B1)!=native.get(0x0AFE_27B1) {
        return Err("BEHAVIOR_COPAN_BASE: base must retain the native plop building type".into());
    }
    for &id in REQUIRED {
        if !active(unit, id) { return Err(format!("BEHAVIOR_COPAN_BASE: service {id:08X}")); }
    }
    if unit.get(super::menu::HASH_PARENT).is_some()
        || FORBIDDEN.iter().any(|&id| active(unit, id))
        || array(unit, 0x0A88_D179)?.iter().any(|v| matches!(v,Value::Int32(13 | 22))) {
        return Err("BEHAVIOR_COPAN_BASE: business or placement residue".into());
    }
    for hash in BUSINESS_INPUTS {
        if unit.get(*hash)!=native.get(*hash) {
            return Err(format!("BEHAVIOR_COPAN_BASE: business input {hash:08X}: {:?}",unit.get(*hash)));
        }
    }
    match program.filter(|p| p.host_base_labor || p.host_parking_staff) {
        Some(program) => super::behavior_container::verify_host_staffing(unit, program),
        None => super::behavior_container::verify(unit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::property_edit::set_array;

    #[test]
    fn legacy_retail_choices_become_malls_without_losing_authored_capacities() {
        use serde_json::json;
        for (profile,tier) in [("ground-commercial-w1-v1",0),("ground-commercial-w3-v1",2)] {
            let mut params=json!({"commercialProfile":profile,"shoppers":271,"goods":913,"jobsW1":21});
            commercial_plop_params(&mut params).unwrap();
            assert_eq!(params["commercialProfile"],super::super::behavior_commercial_mixed::PROFILE);
            for (i,field) in super::super::behavior_commercial_mixed::SHOPPER_INPUTS.into_iter().enumerate() {
                assert_eq!(params[field],json!(if i==tier {271}else{0}));
            }
            assert_eq!(params["goods"],913);
            assert_eq!(params["jobsW1"],21);
        }
        let mut params=json!({"commercialProfile":"ground-commercial-w2-v1","shoppers":77});
        commercial_plop_params(&mut params).unwrap();
        assert_eq!(params["commercialProfile"],super::super::behavior_commercial_mall::PROFILE);
        assert_eq!(params["shoppers"],77);
        assert!(commercial_plop_params(&mut json!({"commercialProfile":"unknown"})).is_err());
    }

    #[test]
    fn component_requires_native_plop_creation_and_rejects_failed_replacement() {
        let bytes=patch(&encode(&PropertyFile::default()).unwrap()).unwrap();
        let unit=PropertyFile::parse(&bytes).unwrap();
        verify_component(&unit).unwrap();
        let mut broken=unit.clone();
        set_array(&mut broken,EXCLUDED,PropType::Key,vec![super::super::behavior_templates::key(0x406C_CC43)]);
        assert!(verify_component(&broken).is_err());
        let mut broken=unit;
        set_scalar(&mut broken,0x07E0_AD09,PropType::Key,super::super::behavior_templates::key(0x1234_5678));
        assert!(verify_component(&broken).is_err());
    }

    #[test]
    fn facility_jobs_stay_on_the_base_when_a_residence_is_attached() {
        use serde_json::json;
        let labor=super::super::behavior_nodes::parse_labor(Some(&json!({"w1":20,"w2":5,"w3":0,"minWorkers":2}))).unwrap();
        let program=compile_building(labor,&[json!({"id":"home","type":"residence","params":{"population":600}})]).unwrap();
        assert!(program.host_base_labor);
        assert_eq!(program.set_float_fields[&0x0A79_17C3],25.0);
        assert_eq!(program.units.len(),1);
        assert_eq!(program.units[0].id,"home");
        assert!(program.parent_template.is_none());
        assert!(program.behavior_category.is_none());
        assert!(program.units.iter().all(|u|u.program.parent_template.is_none()&&u.program.behavior_category.is_none()));
        assert!(program.units[0].program.remove_bundles.contains(&0x3064_5625));
    }

    // Opt-in real-model fixture. Calls the same production API as openscp-build;
    // the model, source package, paths, materials and write/read gates are real.
    #[test]
    #[ignore = "requires OPENSCP_BUILD_FIXTURE_ROOT/SCHEMA/RESULT"]
    fn production_model_fixture_reaches_the_written_package_contract() {
        let root=std::path::PathBuf::from(std::env::var("OPENSCP_BUILD_FIXTURE_ROOT").unwrap());
        let schema=std::env::var("OPENSCP_BUILD_FIXTURE_SCHEMA").unwrap();
        let output=std::env::var("OPENSCP_BUILD_FIXTURE_RESULT").unwrap();
        let raw=std::fs::read_to_string(schema).unwrap();
        let result=super::super::build_from_schema(&root,&raw).unwrap();
        assert_eq!(result.scene.material_resources,6);
        std::fs::write(output,serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    }

    #[test]
    fn four_businesses_and_their_combination_keep_the_host_business_free() {
        use super::super::{behavior,behavior_unit_build,menu,plan,stages::BuildContext,property_edit::set_array};
        use dbpf::ResourceId;
        use serde_json::json;
        let leaves=[
            json!({"id":"home","type":"residence","params":{"residenceProfile":"ground-residence-w1-v1","population":600}}),
            json!({"id":"store","type":"commercial","params":{"commercialProfile":"ground-commercial-w2-v1"}}),
            json!({"id":"visits","type":"tourism","params":{"tourismProfile":"native-tourism-v1"}}),
            json!({"id":"office","type":"office","params":{}}),
        ];
        let extra=["ground-residence-w2-v1","ground-residence-w3-v1","ground-residence-w1-d3-v1","copan-residence-v1","megatower-residence-v1"]
            .map(|profile|vec![json!({"id":"home","type":"residence","params":{"residenceProfile":profile,"population":600}})]);
        let park=json!({"id":"park","type":"park","params":{"residentCapacity":90,"childrenCapacity":80,"visitBudget":960}});
        let park_cases=[vec![park.clone()],vec![park,leaves[1].clone(),leaves[3].clone()],
            vec![json!({"id":"park","type":"park","params":{"residentCapacity":170,"childrenCapacity":120,"visitBudget":2000}}),leaves[0].clone(),leaves[2].clone()]];
        let mixed_case=vec![json!({"id":"three-wealth-store","type":"commercial","params":{
            "commercialProfile":"ground-mall-mixed-v1","shoppersW1":60,"shoppersW2":60,"shoppersW3":60,"goods":750}})];
        for nodes in leaves.iter().map(|n|vec![n.clone()]).chain([leaves.to_vec()]).chain(extra).chain(park_cases).chain([mixed_case]) {
            let schema=json!({"$schema":"openscp.asset","schema_version":2,"id":"graft-test","kind":"building-asset",
                "reference":{"package":"source.package"},"scene":{"document":"asset.lot.json","authoredGround":true},
                "semantics":{"menu":{"title":"Grafts"},"behaviors":nodes},
                "outputs":{"package":"graft.package","mode":"full"}}).to_string();
            let plan=plan::plan_from_schema(&schema).unwrap();
            let program=plan.behavior_program.clone().unwrap();
            assert!(program.copan_plop_base);
            let expected=if nodes.iter().any(|n|n["id"]=="three-wealth-store"){3}else{nodes.len()};
            assert_eq!(program.units.len(),expected);
            assert!(program.units.iter().all(|u|u.shares_building_site && !u.program.author_tourism));
            let host=0x1234_5678;
            let id=|group|ResourceId{type_id:menu::PROPERTY_TYPE,group,instance:host};
            let key=|value|Value::Key(sc_properties::Key{type_id:menu::PROPERTY_TYPE,group:0x40E0_C400,instance:value});
            let empty=encode(&PropertyFile::default()).unwrap();
            let mut ctx=BuildContext::new(".".into(),plan);
            ctx.menu_instance=Some(host);
            ctx.resources.insert(id(menu::UNIT_GROUP),empty.clone());
            ctx.resources.insert(id(menu::MENU_GROUP),empty);
            let mut lot=PropertyFile::default();
            for (hash,ty,values) in [
                (0x0CC8_FE7A,PropType::Key,vec![key(0x418A_18B1)]),
                (0x0CAA_6841,PropType::Int32,vec![Value::Int32(0),Value::Int32(1)]),
                (0x0CAA_6832,PropType::Int32,vec![Value::Int32(0),Value::Int32(1)]),
                (0x0D9A_4E54,PropType::Key,vec![key(0x5757_2DAC)]),
                (0x0D9A_A2BB,PropType::Key,vec![key(0xF0F6_1CED)]),
                (0x0CAA_680D,PropType::Vector3,vec![Value::Vector3([0.0;3]),Value::Vector3([10.0,0.0,0.0])]),
                (0x0CB0_0ED8,PropType::Vector3,vec![Value::Vector3([0.0;3]);2]),
            ] {set_array(&mut lot,hash,ty,values);}
            ctx.resources.insert(id(menu::lot_group()),encode(&lot).unwrap());
            behavior::apply(&mut ctx).unwrap();
            behavior_unit_build::verify(ctx.resources.all(),host,&program).unwrap();
            let mut closed=super::super::package::BuildResources::from_source(ctx.resources.all().clone());
            closed.retain_identity_closure(host);
            behavior_unit_build::verify(closed.all(),host,&program).unwrap();
            let base=PropertyFile::parse(ctx.resources.get(&id(menu::UNIT_GROUP)).unwrap()).unwrap();
            verify(&base,Some(&program)).unwrap();
            for scope in &program.units {
                if scope.program.copan_park_core {
                    let child_id=ResourceId{instance:behavior_unit_build::instance(host,&scope.id),..id(menu::UNIT_GROUP)};
                    let child=PropertyFile::parse(ctx.resources.get(&child_id).unwrap()).unwrap();
                    super::super::behavior_park_core::verify(ctx.resources.all(),&child,child_id.instance,&scope.program).unwrap();
                    let mut broken=child.clone();broken.values.retain(|p|p.hash!=0x0B98_805A);
                    assert!(super::super::behavior_park_core::verify(ctx.resources.all(),&broken,child_id.instance,&scope.program).is_err());
                    let mut broken=child.clone();set_array(&mut broken,EXCLUDED,PropType::Key,vec![]);
                    assert!(super::super::behavior_business_services::verify(&broken).is_err());
                }
                if scope.program.templates.residence() {
                    let child_id=ResourceId{instance:behavior_unit_build::instance(host,&scope.id),..id(menu::UNIT_GROUP)};
                    let mut child=PropertyFile::parse(ctx.resources.get(&child_id).unwrap()).unwrap();
                    verify_residence_component(&child).unwrap();
                    super::super::behavior_copan_residence::verify(ctx.resources.all(),child_id.instance,&scope.program).unwrap();
                    let mut missing=ctx.resources.all().clone();
                    let mut empty_column=child.clone();
                    set_array(&mut empty_column,0x0A02_09A4,PropType::Key,vec![]);
                    missing.insert(child_id,encode(&empty_column).unwrap());
                    assert!(super::super::behavior_copan_residence::verify(&missing,child_id.instance,&scope.program).is_err());
                    let mut missing=ctx.resources.all().clone();
                    let private=super::super::behavior_copan_residence::bundle_ids(child_id.instance)[0];
                    missing.remove(&private);
                    assert!(super::super::behavior_copan_residence::verify(&missing,child_id.instance,&scope.program).is_err());
                    let mut broken=ctx.resources.all().clone();
                    let mut callbacks=PropertyFile::parse(&broken[&private]).unwrap();
                    callbacks.values.retain(|p|p.hash!=0x0C61_4160);
                    broken.insert(private,encode(&callbacks).unwrap());
                    assert!(super::super::behavior_copan_residence::verify(&broken,child_id.instance,&scope.program).is_err());
                    let exclusions=array(&child,EXCLUDED).unwrap().iter().filter(|v|!matches!(v,Value::Key(k) if k.instance==0x3064_5625)).cloned().collect();
                    set_array(&mut child,EXCLUDED,PropType::Key,exclusions);
                    assert!(verify_residence_component(&child).is_err());
                }
            }
            assert_eq!(base.get(0x0A79_17CB),source().unwrap().get(0x0A79_17CB));
            assert!(!FORBIDDEN.iter().any(|&b|active(&base,b)));
        }
    }

    #[test]
    fn authored_core_uses_copan_base_through_the_production_plan_and_written_verifier() {
        use super::super::{behavior, behavior_unit_build, connector::Resources, menu, plan, stages::BuildContext};
        use dbpf::ResourceId;
        for labor in [serde_json::Value::Null,serde_json::json!({"w1":10,"w2":0,"w3":0,"minWorkers":1})] {
            let schema=serde_json::json!({"$schema":"openscp.asset","schema_version":2,"id":"base-test",
                "kind":"building-asset","reference":{"package":"source.package"},
                "scene":{"document":"asset.lot.json","authoredGround":true},
                "semantics":{"menu":{"title":"Base"},"labor":labor,
                    "behaviors":[{"id":"core","type":"utilities-core","params":{}}]},
                "outputs":{"package":"base.package","mode":"full"}}).to_string();
            let plan=plan::plan_from_schema(&schema).unwrap();
            let program=plan.behavior_program.clone().unwrap();
            assert!(program.copan_plop_base);
            assert!(program.units.is_empty());
            assert_eq!(plan.processor_version,VERSION);
            let host=0x1234_5678;
            let id=|group|ResourceId{type_id:menu::PROPERTY_TYPE,group,instance:host};
            let empty=encode(&PropertyFile::default()).unwrap();
            let mut ctx=BuildContext::new(".".into(),plan);
            ctx.menu_instance=Some(host);
            let resources:Resources=[menu::UNIT_GROUP,menu::MENU_GROUP,menu::lot_group()].into_iter()
                .map(|group|(id(group),empty.clone())).collect();
            ctx.resources=super::super::package::BuildResources::from_source(resources);
            behavior::apply(&mut ctx).unwrap();
            behavior_unit_build::verify(ctx.resources.all(),host,&program).unwrap();
            let unit=PropertyFile::parse(ctx.resources.get(&id(menu::UNIT_GROUP)).unwrap()).unwrap();
            assert_eq!(unit.get(0x0A79_17C5).and_then(|p|p.scalar()),
                Some(&Value::Float(if program.host_base_labor {10.0}else{0.0})));
            let mut broken=ctx.resources.all().clone();
            let mut unit=unit;
            set_array(&mut unit,EXCLUDED,PropType::Key,vec![super::super::behavior_templates::key(0x3064_5625)]);
            broken.insert(id(menu::UNIT_GROUP),encode(&unit).unwrap());
            assert!(behavior_unit_build::verify(&broken,host,&program).is_err());
        }
    }

    #[test]
    fn copan_base_retains_all_services_and_rejects_residential_state_or_missing_sewage() {
        let empty = encode(&PropertyFile::default()).unwrap();
        let mut unit = PropertyFile::parse(&patch(&empty).unwrap()).unwrap();
        verify(&unit,None).unwrap();
        let native=source().unwrap();
        for h in [0x0AFB_9882,0x0B88_904E,0x0C09_DAA0,0x0C09_DA6A,0x0C09_DA65,0x0C09_DA5A] {
            assert_eq!(unit.get(h),native.get(h));
        }
        set_array(&mut unit,EXCLUDED,PropType::Key,vec![super::super::behavior_templates::key(0x1B94_6073)]);
        assert!(verify(&unit,None).unwrap_err().contains("1B946073"));
        let mut unit=PropertyFile::parse(&patch(&empty).unwrap()).unwrap();
        set_array(&mut unit,0x0A88_D179,PropType::Int32,vec![Value::Int32(13)]);
        assert!(verify(&unit,None).is_err());
    }
}
