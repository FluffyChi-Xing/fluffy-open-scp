//! The plop host owns the complete native external service budget.
//! Business components own no utility consumer, sink, storage or demand.
//! Their activation follows the connected host; utility faults belong to the host.
use super::{behavior_nodes::ResolvedBehavior, connector::Resources, menu};
use sc_properties::{PropertyFile, Value};
use serde::Serialize;

const POWER: [u32; 4] = [0x0B88_904E, 0x0C09_DAA0, 0x0C09_DAB2, 0x0AFB_9882];
const WATER: [u32; 4] = [0x0C09_DA65, 0x0C09_DA5A, 0x0C09_DA6E, 0x0C09_DA6A];
const ERROR: &str = "BEHAVIOR_COPAN_SERVICES";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct Budget {
    power: u32,
    water: u32,
    power_rate: u32,
    water_rate: u32,
}

fn integer(value: f64, field: u32) -> Result<u32, String> {
    if value < 1.0 || value > i32::MAX as f64 || value.fract() != 0.0
        || (value as f32) as f64 != value {
        return Err(format!("{ERROR}: {field:08X} must be a positive exactly representable native integer"));
    }
    Ok(value as u32)
}

fn value(file: &PropertyFile, field: u32) -> Result<u32, String> {
    match file.get(field).and_then(|p| p.scalar()) {
        Some(Value::Float(value)) => integer(*value as f64, field),
        _ => Err(format!("{ERROR}: missing Float {field:08X}")),
    }
}

fn planned(program: &ResolvedBehavior, source: &PropertyFile, field: u32) -> Result<u32, String> {
    match program.set_float_fields.get(&field) {
        Some(value) => integer(*value, field),
        None => value(source, field),
    }
}

fn set(program: &mut ResolvedBehavior, field: u32, value: u32) -> Result<(), String> {
    integer(value as f64, field)?;
    program.set_float_fields.insert(field, value as f64);
    if !program.declared_unit_columns.contains(&field) {
        program.declared_unit_columns.push(field);
    }
    Ok(())
}

fn assign(program: &mut ResolvedBehavior, fields: [u32; 4], amount: u32, rate: u32) -> Result<(), String> {
    let capacity = amount.checked_mul(3).ok_or_else(||format!("{ERROR}: capacity overflow"))?;
    for (field, value) in fields.into_iter().zip([amount, capacity, capacity, rate]) {
        set(program, field, value)?;
    }
    Ok(())
}

pub(super) fn allocate(program: &mut ResolvedBehavior) -> Result<(), String> {
    if program.units.is_empty() { return Ok(()); }
    let source = super::behavior_copan_base::source()?;
    let power = planned(program, &source, POWER[0])?;
    let water = planned(program, &source, WATER[0])?;
    let power_rate = planned(program, &source, POWER[3])?;
    let water_rate = planned(program, &source, WATER[3])?;
    for (fields, amount) in [(POWER,power),(WATER,water)] {
        let capacity=amount.checked_mul(3).ok_or_else(||format!("{ERROR}: capacity overflow"))?;
        for field in [fields[1],fields[2]] {
            if planned(program,&source,field)?!=capacity {
                return Err(format!("{ERROR}: {field:08X} must retain the native 3x amount binding"));
            }
        }
    }
    assign(program,POWER,power,power_rate)?;
    assign(program,WATER,water,water_rate)?;
    for scope in &mut program.units {
        for field in POWER.into_iter().chain(WATER) {
            scope.program.set_float_fields.remove(&field);
            scope.program.declared_unit_columns.retain(|h|*h!=field);
        }
    }
    program.copan_service_budget=Some(Budget{power,water,power_rate,water_rate});
    Ok(())
}

fn verify_host(unit: &PropertyFile, planned: &ResolvedBehavior) -> Result<(),String> {
    super::behavior_container::verify_host_utility_connections(unit)?;
    for fields in [POWER,WATER] {
        let amount=value(unit,fields[0])?;
        let capacity=amount.checked_mul(3).ok_or_else(||format!("{ERROR}: capacity overflow"))?;
        for (field,expected) in fields.into_iter().zip([amount,capacity,capacity,value(unit,fields[3])?]) {
            if value(unit,field)?!=expected || planned.set_float_fields.get(&field)!=Some(&(expected as f64)) {
                return Err(format!("{ERROR}: consumer differs from allocation at {field:08X}"));
            }
        }
    }
    Ok(())
}

const CONSUMERS:[u32;4]=[0x4893_9D96,0x825B_4CA4,0x19E5_DB54,0xD606_8BEA];

pub(super) fn patch_child(unit:&mut PropertyFile,child:u32)->Result<Resources,String>{
    use super::{behavior_templates::{array,EXCLUDED,key},property_edit::set_array_with_flags};
    use sc_properties::PropType;
    let mut excluded=array(unit,EXCLUDED)?.to_vec();
    for id in CONSUMERS {if !excluded.contains(&key(id)){excluded.push(key(id));}}
    set_array_with_flags(unit,EXCLUDED,PropType::Key,excluded,0x809C);
    unit.values.retain(|p|!POWER.contains(&p.hash)&&!WATER.contains(&p.hash));
    super::behavior_utility_state::apply(unit,child)
}
pub(super) fn verify_child(resources:&Resources,unit:&PropertyFile,child:u32)->Result<(),String>{
    use super::behavior_templates::{contains,ENABLE,EXCLUDED};
    for id in CONSUMERS {
        if !contains(unit,&[EXCLUDED],id) || (contains(unit,&ENABLE,id)&&!contains(unit,&[EXCLUDED],id)) {
            return Err(format!("{ERROR}: child owns utility consumer {id:08X}"));
        }
    }
    if unit.values.iter().any(|p|POWER.contains(&p.hash)||WATER.contains(&p.hash)) {
        return Err(format!("{ERROR}: child owns utility demand/storage inputs"));
    }
    super::behavior_utility_state::verify(resources,unit,child)
}
pub(super) fn verify(resources: &Resources, host: u32, program: &ResolvedBehavior) -> Result<(),String> {
    let budget=program.copan_service_budget.as_ref().ok_or_else(||format!("{ERROR}: missing whole-building budget"))?;
    let key=|instance|dbpf::ResourceId{type_id:menu::PROPERTY_TYPE,group:menu::UNIT_GROUP,instance};
    let unit=PropertyFile::parse(resources.get(&key(host)).ok_or_else(||format!("{ERROR}: missing host"))?).map_err(|e|e.to_string())?;
    verify_host(&unit,program)?;
    for (fields,amount,rate) in [(POWER,budget.power,budget.power_rate),(WATER,budget.water,budget.water_rate)] {
        for (field,expected) in fields.into_iter().zip([amount,amount*3,amount*3,rate]) {
            if value(&unit,field)?!=expected {return Err(format!("{ERROR}: host lost whole-building service budget"));}
        }
    }
    for scope in &program.units {
        let child=super::behavior_unit_build::instance(host,&scope.id);
        let unit=PropertyFile::parse(resources.get(&key(child)).ok_or_else(||format!("{ERROR}: missing child"))?).map_err(|e|e.to_string())?;
        verify_child(resources,&unit,child)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leaf_count_never_divides_or_manufactures_host_demand(){
        for count in [1,2,4,10,20] {
            let mut program=ResolvedBehavior::default();
            for i in 0..count{program.units.push(super::super::behavior_units::UnitBehavior{
                id:i.to_string(),program:ResolvedBehavior::default(),shares_building_site:true});}
            allocate(&mut program).unwrap();
            for (fields,total) in [(POWER,12.0),(WATER,10.0)] {
                assert_eq!(program.set_float_fields[&fields[0]],total);
                assert_eq!(program.set_float_fields[&fields[1]],total*3.0);
                for scope in &program.units{assert!(fields.iter().all(|h|!scope.program.set_float_fields.contains_key(h)));}
            }
        }
    }
    #[test]
    fn missing_adapter_and_reintroduced_consumers_are_rejected(){
        let child=0x1234_5678;let mut unit=super::super::behavior_copan_base::source().unwrap();
        let resources=patch_child(&mut unit,child).unwrap();
        verify_child(&resources,&unit,child).unwrap();
        assert!(verify_child(&Resources::new(),&unit,child).is_err());
        let mut bad=unit.clone();
        super::super::property_edit::set_scalar(&mut bad,POWER[0],sc_properties::PropType::Float,Value::Float(1.0));
        assert!(verify_child(&resources,&bad,child).is_err());
        let mut bad=unit;let row=bad.values.iter_mut().find(|p|p.hash==super::super::behavior_templates::EXCLUDED).unwrap();
        if let sc_properties::Kind::Array(values)=&mut row.kind{values.retain(|v|!matches!(v,Value::Key(k) if k.instance==CONSUMERS[0]));}
        assert!(verify_child(&resources,&bad,child).is_err());
    }
}
