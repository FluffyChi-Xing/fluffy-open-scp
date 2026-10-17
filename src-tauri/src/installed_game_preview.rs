//! @summary Build static authoring previews from original installed declarative resources, never imported replicas.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropertyFile, Text, Value as PropValue};
use serde_json::{Value, json};
use crate::{activity::CommandError, local_game_assets::{ProviderSession, available_locales, err, fail, fnv, normalize_locale, read, typed, MAX_RESOURCE}};

const PROP: u32=0x00B1B104;
const LAYOUT: u32=0x67771F5C;
const LAYOUT_GROUP:u32=0x0B074E5A;
const CATEGORY_IDS:u32=0x09A4A024;
const CATEGORY_DEBUG:u32=0x0D115956;
const CATEGORY_NAME:u32=0x0C2BF463;
const CATEGORY_ICONS:u32=0x0AB8C50C;
const CATEGORY_LAYOUT:u32=0x0C22F4DE;
const TOOL_LIST:u32=0x0AFBA88D;
const TITLE:u32=0x0A09F5FA;
const DESCRIPTION:u32=0x0A09F5FB;
const UI_CATEGORY:u32=0x0DB9FC63;
const UI_POSITION:u32=0x0DC1E3E0;
const PARENT:u32=0x00B2CCCB;
const TOOL_ICON:u32=0x0977AA8F;
const MARQUEE:u32=0x0DDEEE56;
const UNLOCK:u32=0x0DE84DDC;
const HARD_GATE:u32=0x0975695F;
const EFFECT_TITLE:u32=0x0EB1FC05;

/// @summary Native control identifiers selected for an explicit static city-HUD authoring scene.
const EDITOR_HUD_CONTROLS:&[u64]=&[241,772,1018,788,1459,119,24,971,1273,1367,1364,1079,42,46];

/// @summary Preserve the upstream authoring category order rather than showing additional runtime categories.
const EDITOR_CATEGORY_IDS:&[u32]=&[0xAC12C904,0x3193C0BE,0x8E8B4E10,0x9E3C3DFA,0xCD1DDC35,0x674129FD,0x7CB23A4E,0x9FAC79C9,0xDB2D51A3,0x1159ECD7,0x95A9D6FD,0xCBC27856,0x899BEA70,0x9B7C811F];

/// @summary Preserve upstream's ordered park authoring references; labels and all game payloads resolve from the installation.
const EDITOR_PARK_IDS:&[u32]=&[0x152D57D0,0x4A81FBCD,0x62E6FE38,0xB9053717,0xC77D92F5,0xEEE9D42A,0x46173A50,0x646AAA32,0x7A90E787,0x8C673576,0x93909568,0xCD62B110,0xDC02CAED,0xDD009DE5,0x28E3DACC,0x49CC1D7B,0x18E967A0,0x2AF606C8,0x46173A53,0x70724BF4,0xB2D783E1,0xD44673F9,0x05CAC7B5,0x22404DF3,0x6DE007F7,0x70724BF7,0x83382FD4,0xC0A75B3A,0x1230E4D8,0x1230E4D9,0x3D965136,0x7F2BE24E,0x908629EA,0x9D122A62,0xA317C2FB,0xB5C6DD6A,0xE7FE22E3,0x4FF4E214,0x9EC2E438,0xA94AAF37,0xBBF5F301,0x193BC89C,0x83ABE66B,0x175AF694,0x175AF695,0xEDF237F1,0x41CD00BB,0x17108AFD,0x8F5E7184,0xBD973244,0xE8D7CA3D,0xC0EC9BBC,0xD0818C91,0x9133454C,0x505B4074,0xA1BA832F];

/// @summary Project original controls into a declared editor scene; never infer the game's controller visibility state.
fn project_editor_hud(mut root:Value)->Result<(Value,Value),CommandError>{
    let children=root.get_mut("children").and_then(Value::as_array_mut).ok_or_else(||fail("Original global UI has no child controls"))?;
    let matches:Vec<_>=children.iter().enumerate().filter(|(_,n)|n.get("instanceID").and_then(Value::as_u64)==Some(795)).map(|(i,_)|i).collect();
    if matches.len()!=1{return Err(fail("Original Shared UI parent 795 is missing or ambiguous; editor projection cannot be applied"))}
    let mut shared=children[matches[0]].clone();let excluded_roots:Vec<_>=children.iter().filter(|n|n.get("instanceID").and_then(Value::as_u64)!=Some(795)).map(|n|json!({"instanceID":n.get("instanceID"),"comment":n.get("comment"),"reason":"runtime-controlled scene, dialog, overlay or non-HUD root"})).collect();
    let shared_children=shared.get_mut("children").and_then(Value::as_array_mut).ok_or_else(||fail("Original Shared UI parent has no control list"))?;
    for id in EDITOR_HUD_CONTROLS{if shared_children.iter().filter(|n|n.get("instanceID").and_then(Value::as_u64)==Some(*id)).count()!=1{return Err(fail(format!("Required original HUD control {id} is missing or ambiguous")))}}
    let excluded_shared:Vec<_>=shared_children.iter().filter(|n|!n.get("instanceID").and_then(Value::as_u64).is_some_and(|id|EDITOR_HUD_CONTROLS.contains(&id))).map(|n|json!({"instanceID":n.get("instanceID"),"comment":n.get("comment"),"reason":"not selected in static editor HUD scene; live controller state unavailable"})).collect();
    shared_children.retain(|n|n.get("instanceID").and_then(Value::as_u64).is_some_and(|id|EDITOR_HUD_CONTROLS.contains(&id)));
    project_upstream_controls(&mut shared,&mut vec![795])?;
    *children=vec![shared];
    let projection=json!({"scene":"city-hud-authoring","policyVersion":1,"sourceResource":"installed-game:67771F5C:0B074E5A:90D2CCCC","verifiedRuntimeState":false,"retainedSharedControlIds":EDITOR_HUD_CONTROLS,"excludedRoots":excluded_roots,"excludedSharedControls":excluded_shared,"geometry":"original retained nodes and coordinates are unchanged"});
    root["editorProjection"]=projection.clone();Ok((root,projection))
}

/// @summary Apply upstream editor scene selections by numeric control references, retaining original installed payloads.
fn project_upstream_controls(node:&mut Value,path:&mut Vec<u64>)->Result<(),CommandError>{
    if path.as_slice()==[795,241]{if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){children.retain(|child|child.get("instanceID").and_then(Value::as_u64)!=Some(100));}}
    let excluded=matches!(path.as_slice(),[795,119,118]|[795,42,1395,1394]|[795,42,1907,1908]|[795,42,855,1061]|[795,42,855,1059]|[795,42,855,1057]);
    if excluded{if let Some(path)=node.get("layoutPath").cloned(){node["_module"]=path;}node["_missing"]=json!(true);node["children"]=json!([]);return Ok(())}
    if path.as_slice()==[795,42,132]{node["layoutPath"]=json!("Layouts/Palette/SpeedPanel.js");node["type"]=json!("cLayout");}
    if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{let id=child.get("instanceID").and_then(Value::as_u64).ok_or_else(||fail("Original editor control lacks numeric instance identity"))?;path.push(id);project_upstream_controls(child,path)?;path.pop();}}Ok(())
}

/// @summary Preserve upstream palette scene selections using numeric paths, resolving retained controls from installed layouts.
fn project_palette_controls(node:&mut Value,path:&mut Vec<u64>,category:u32)->Result<(),CommandError>{
    let retained:Option<&[u64]>=match (category,path.as_slice()){
        (0xAC12C904,[2])=>Some(&[9,11,188,189,26,179,31,190,24,32,192,25,183,218]),
        (0x3193C0BE,[2])=>Some(&[9,11,49,50,69,210,250,64,230]),
        (0x3193C0BE,[2,49])=>Some(&[46,34,209]),(0x3193C0BE,[2,49,209])=>Some(&[121]),
        (0x3193C0BE,[2,50])=>Some(&[208]),(0x3193C0BE,[2,50,208])=>Some(&[77]),
        (0x3193C0BE,[2,69])=>Some(&[207]),(0x3193C0BE,[2,69,207])=>Some(&[32,52,35,173]),
        (0x3193C0BE,[2,210])=>Some(&[112]),
        (0x674129FD,[2])=>Some(&[32,11,9,31,100,101,16,169,107,26]),
        (0xCBC27856|0x899BEA70,[2])=>Some(&[13,4,169,167,25,5,19,10,242,166,37,252]),
        (0x9B7C811F,[2])=>Some(&[32,9,11,8,26,31]),(0x9B7C811F,[2,31])=>Some(&[]),
        (_,[2])=>Some(&[32,11,9,31,100,101,26,169,107]),_=>None,
    };
    if let (Some(retained),Some(children))=(retained,node.get_mut("children").and_then(Value::as_array_mut)){children.retain(|child|child.get("instanceID").and_then(Value::as_u64).is_some_and(|id|retained.contains(&id)));}
    if let Some(module)=node.get("layoutPath").and_then(Value::as_str).map(str::to_owned){if matches!(module.rsplit('/').next(),Some("PaletteContextualInfo.js"|"InfoButton.js")){node["_module"]=json!(module);node["_missing"]=json!(true);node["children"]=json!([]);return Ok(())}}
    if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{let id=child.get("instanceID").and_then(Value::as_u64).ok_or_else(||fail("Original palette control lacks numeric identity"))?;path.push(id);project_palette_controls(child,path,category)?;path.pop();}}Ok(())
}

/// @summary Read an original declarative resource as JSON only; its nominal JavaScript type is never evaluated.
fn original_json(session:&ProviderSession,id:ResourceId)->Result<Value,CommandError>{
    let bytes=read(session,id)?;let payload=bytes.strip_prefix(&[0xEF,0xBB,0xBF]).unwrap_or(&bytes);
    let value:Value=serde_json::from_slice(payload).map_err(|e|fail(format!("Original declarative JSON is invalid ({id}): {e}")))?;
    if !value.is_object(){return Err(fail(format!("Original layout is not an object: {id}")))}Ok(value)
}
/// @summary Resolve source module filenames using the audited layout group and game filename-stem convention.
fn layout_id(path:&str)->Result<ResourceId,CommandError>{
    if path.len()>512||!path.is_ascii()||path.contains([':', '\0','?','#','%']){return Err(fail("Invalid original layout reference"))}
    let name=path.rsplit(['/', '\\']).next().unwrap_or_default();let stem=name.strip_suffix(".js").ok_or_else(||fail(format!("Unsupported declarative layout reference: {path}")))?;
    if stem.is_empty()||!stem.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b' ')){return Err(fail("Invalid layout filename"))}
    // @summary Original upstream module labels resolve to these verified installed layouts; filename hashes identify other variants.
    let instance=match stem.to_ascii_lowercase().as_str(){"categorybuttonwithicon2"=>0x33B37BAE,"budgetbutton"=>0xDE286E0E,_=>fnv(stem)};
    Ok(ResourceId{type_id:LAYOUT,group:LAYOUT_GROUP,instance})
}
/// @summary Apply an original module root and explicit instance overrides, preserving authored wrapper identity.
fn merge_module(template:Value,wrapper:&serde_json::Map<String,Value>,path:&str)->Result<Value,CommandError>{
    let mut result=template.as_object().ok_or_else(||fail("Module template is not an object"))?.clone();
    let mut children=result.get("children").and_then(Value::as_array).cloned().unwrap_or_default();
    if let Some(extra)=wrapper.get("children").and_then(Value::as_array){children.extend(extra.iter().cloned());}
    for (key,value) in wrapper{if key!="children"&&key!="exposedProperties"{result.insert(key.clone(),value.clone());}}
    result.insert("children".into(),Value::Array(children));result.insert("_module".into(),Value::String(path.into()));Ok(Value::Object(result))
}
/// @summary Inline only declared layoutPath modules, with exact cycle, depth and output-node limits.
fn inline(session:&ProviderSession,node:Value,stack:&mut Vec<ResourceId>,count:&mut usize)->Result<Value,CommandError>{
    *count+=1;if *count>30000||stack.len()>64{return Err(fail("Original layout graph exceeds preview bounds"))}
    let mut node=node;
    if let Some(path)=node.get("layoutPath").and_then(Value::as_str).filter(|s|!s.is_empty()&&!s.eq_ignore_ascii_case("none")).filter(|_|node.get("_missing")!=Some(&Value::Bool(true))&&node.get("_module").is_none()).map(str::to_owned){
        let id=layout_id(&path)?;if stack.contains(&id){return Err(fail(format!("Original layout module cycle at {path}")))}
        stack.push(id);let template=original_json(session,id)?;let merged=merge_module(template,node.as_object().ok_or_else(||fail("Invalid layout node"))?,&path)?;
        node=inline(session,merged,stack,count)?;stack.pop();return Ok(node)
    }
    if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{*child=inline(session,child.take(),stack,count)?;}}
    Ok(node)
}
/// @summary Preserve upstream editor instance namespaces, allowing existing selection and override references to round-trip.
fn namespace_modules(node:&mut Value,palette:bool){
    /// @summary Namespace original descendant identifiers without changing their geometry or payload fields.
    fn rename(node:&mut Value,prefix:&str){if let Some(id)=node.get("instanceID"){let id=id.as_str().map(str::to_owned).unwrap_or_else(||id.to_string());node["instanceID"]=json!(format!("{prefix}::{id}"));}if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{rename(child,prefix)}}}
    if let Some(module)=node.get("_module").and_then(Value::as_str).map(str::to_owned){
        if node.get("_missing")==Some(&Value::Bool(true)){return}
        let id=node.get("instanceID").map(|v|v.as_str().map(str::to_owned).unwrap_or_else(||v.to_string())).unwrap_or_default();let prefix=if palette{format!("{id}|{}",module.rsplit('/').next().unwrap_or(&module))}else{id};
        if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{rename(child,&prefix)}}
        if module.ends_with("/SpeedPanel.js"){node["_module"]=json!("attach:154B8B98");node.as_object_mut().unwrap().remove("layoutPath");}
    }else if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{namespace_modules(child,palette)}}
}
/// @summary Expose scalar and array properties uniformly without inventing absent values.
fn values(file:&PropertyFile,hash:u32)->Vec<&PropValue>{match file.get(hash).map(|p|&p.kind){Some(Kind::Scalar(v))=>vec![v],Some(Kind::Array(v))=>v.iter().collect(),_=>Vec::new()}}
/// @summary Preserve full TGI keys read from actual property payloads.
fn keys(file:&PropertyFile,hash:u32)->Vec<Key>{values(file,hash).into_iter().filter_map(|v|if let PropValue::Key(k)=v{Some(*k)}else{None}).collect()}
/// @summary Preserve locale table identity carried by a Text property.
fn texts(file:&PropertyFile,hash:u32)->Vec<Text>{values(file,hash).into_iter().filter_map(|v|if let PropValue::Text(t)=v{Some(*t)}else{None}).collect()}
/// @summary Read an ordered numeric property without silently substituting a different type.
fn number(file:&PropertyFile,hash:u32)->Option<i64>{values(file,hash).first().and_then(|v|match v{PropValue::Int32(n)=>Some(i64::from(*n)),PropValue::UInt32(n)=>Some(i64::from(*n)),_=>None})}
/// @summary Original property files indexed by their complete resource identifiers.
type Properties=HashMap<ResourceId,PropertyFile>;
/// @summary Decode candidate UI/property dependencies from installed packages with explicit parser errors.
fn properties(session:&ProviderSession)->Result<Properties,CommandError>{
    let mut result=HashMap::new();let mut ids:Vec<_>=session.index.keys().filter(|id|id.type_id==PROP&&matches!(id.group&0xFFFF,0xBA02|0xBA03|0x8A01|0xC900)).copied().collect();ids.sort_by_key(|id|(id.group,id.instance));
    while let Some(id)=ids.pop(){if result.contains_key(&id){continue}let bytes=read(session,id)?;let file=PropertyFile::parse(&bytes).map_err(|e|fail(format!("Installed UI property parse failed ({id}): {e}")))?;
        for key in keys(&file,PARENT){let exact=ResourceId{type_id:if key.type_id==0{PROP}else{key.type_id},group:key.group,instance:key.instance};let parent=if session.index.contains_key(&exact){exact}else{let candidates:Vec<_>=session.index.keys().filter(|id|id.type_id==PROP&&id.instance==key.instance&&(key.group==0||id.group==key.group)).copied().collect();if candidates.len()!=1{return Err(fail(format!("UI inheritance dependency unavailable or ambiguous: {exact}")))}candidates[0]};if !result.contains_key(&parent){ids.push(parent)}}result.insert(id,file);
    }Ok(result)
}
/// @summary Resolve exact property keys, permitting group-zero instance references only when unambiguous.
fn property<'a>(files:&'a Properties,key:Key)->Result<(ResourceId,&'a PropertyFile),CommandError>{
    let id=ResourceId{type_id:if key.type_id==0{PROP}else{key.type_id},group:key.group,instance:key.instance};
    if let Some(file)=files.get(&id){return Ok((id,file))}
    if key.group!=0{return Err(fail(format!("Required installed property is missing: {id}")))}
    let matches:Vec<_>=files.iter().filter(|(id,_)|id.type_id==PROP&&id.instance==key.instance).collect();
    if matches.len()!=1{return Err(fail(format!("Required property instance {:08X} is missing or ambiguous",key.instance)))}Ok((*matches[0].0,matches[0].1))
}
/// @summary Compare complete identities; group-zero menu references resolve within the expected native instance-type family.
fn references(session:&ProviderSession,items:&[Key],target:ResourceId)->Result<bool,CommandError>{
    for key in items{
        let kind=if key.type_id==0{PROP}else{key.type_id};if kind!=target.type_id||key.instance!=target.instance{continue}
        if key.group!=0{if key.group==target.group{return Ok(true)}continue}
        let candidates:Vec<_>=session.index.keys().filter(|id|id.type_id==kind&&id.instance==key.instance&&(id.group&0xFFFF)==(target.group&0xFFFF)).copied().collect();
        if candidates.len()!=1{return Err(fail(format!("Ambiguous group-zero menu reference: {:08X}",key.instance)))}
        if candidates[0]==target{return Ok(true)}
    }Ok(false)
}
/// @summary Resolve property inheritance with cycle/missing-parent diagnostics rather than partial silent flattening.
fn inherited(files:&Properties,key:Key,stack:&mut Vec<ResourceId>)->Result<PropertyFile,CommandError>{
    let (id,file)=property(files,key)?;if stack.len()>64||stack.contains(&id){return Err(fail(format!("Property inheritance cycle/depth at {id}")))}
    let mut merged=file.clone();if let Some(parent)=keys(file,PARENT).first(){stack.push(id);let parent=inherited(files,*parent,stack)?;stack.pop();for field in parent.values{if merged.get(field.hash).is_none(){merged.values.push(field)}}}Ok(merged)
}

/// @summary Load only the explicitly requested installed language, retaining original string-table IDs.
fn locales(session:&ProviderSession,locale:&str)->Result<BTreeMap<u32,BTreeMap<u32,String>>,CommandError>{
    if !available_locales(session).contains(&locale.to_owned()){return Err(CommandError::new("locale_unavailable",format!("Requested locale {locale} is not installed; available: {}",available_locales(session).join(", "))))}
    let mut tables=BTreeMap::new();
    for package in &session.packages{
        let parts:Vec<_>=package.path().components().map(|p|p.as_os_str().to_string_lossy().into_owned()).collect();
        let selected=parts.iter().position(|s|s.eq_ignore_ascii_case("Locale")).and_then(|pos|parts.get(pos+1)).is_some_and(|name|normalize_locale(name)==locale);if !selected{continue}
        for entry in package.entries().iter().filter(|e|e.id.type_id==0x0A98EAF0){
            if u64::from(entry.decompressed_size)>MAX_RESOURCE{return Err(fail("Locale table exceeds 16 MiB"))}let bytes=package.read(entry).map_err(err)?;
            let parsed=sc_properties::parse_string_table(&bytes).map_err(|e|fail(format!("Installed locale table {:08X} is invalid: {e}",entry.id.instance)))?;
            tables.insert(entry.id.instance,parsed.into_iter().collect());
        }
    }
    if tables.is_empty(){return Err(fail("Selected installed locale contains no readable string tables"))}Ok(tables)
}
/// @summary Resolve a real Text reference, preserving missing references as explicit dependency failures.
fn localized(tables:&BTreeMap<u32,BTreeMap<u32,String>>,text:Text)->Result<String,CommandError>{tables.get(&text.table_id).and_then(|t|t.get(&text.instance_id)).map(|text|plain_text(text)).ok_or_else(||fail(format!("Required locale text is missing: {:08X}:{:08X}",text.table_id,text.instance_id)))}
/// @summary Remove original presentation tags as inert text, without executing or inserting source HTML.
fn plain_text(text:&str)->String{let mut plain=String::new();let mut tag=false;for c in text.chars(){match c{'<'=>tag=true,'>'=>tag=false,_ if !tag=>plain.push(c),_=>{}}}plain}
/// @summary Extract inert authoring statistic labels and literal values; runtime substitutions remain unavailable.
fn static_statistic(text:&str)->Value{
    let plain=plain_text(text);
    let (label,value)=plain.split_once([':', '：']).map(|(label,value)|(label.trim_end(),Some(value.trim()))).unwrap_or((plain.trim_end(),None));
    let value=value.filter(|value|!value.is_empty()&&!value.contains('~')&&!value.contains('{')&&!value.contains('}'));
    json!({"label":label,"value":value})
}
/// @summary Select upstream statistic references by complete locale table and instance identity.
fn is_editor_statistic(text:Text)->bool{!matches!((text.table_id,text.instance_id),(0xFCFEA360,0x0FFBA772|0x0FFE4F6F)|(0x2FAE9986,0x0EF3E7F5))}
/// @summary Preserve upstream tool text semantics: one reference in the field's supported source table, otherwise an empty authoring field.
fn editor_text_reference(references:&[Text],table:u32)->Option<Text>{match references{[text] if text.table_id==table=>Some(*text),_=>None}}
/// @summary Resolve only the original editor's supported tool text references without broadening its DLC or multi-condition behavior.
fn editor_tool_text(tables:&BTreeMap<u32,BTreeMap<u32,String>>,file:&PropertyFile,hash:u32,table:u32)->Result<String,CommandError>{editor_text_reference(&texts(file,hash),table).map(|text|localized(tables,text)).transpose().map(Option::unwrap_or_default)}
/// @summary Decode original numeric/string locale IDs into the renderer's stable hexadecimal keys.
fn locale_id(value:&Value,filename:bool)->Option<u32>{if let Some(n)=value.as_u64(){return u32::try_from(n).ok()}let s=value.as_str()?;if filename&&s.ends_with(".json"){return Some(fnv(s.rsplit(['/', '\\']).next()?.strip_suffix(".json")?))}if let Some(h)=s.strip_prefix("0x").or_else(||s.strip_prefix("0X")){return u32::from_str_radix(h,16).ok()}s.parse().ok()}
/// @summary Normalize layout locale pointers without importing any translated labels from bundled data.
fn normalize_text_pointers(node:&mut Value)->Result<(),CommandError>{
    if let Some(pointer)=node.get_mut("localeString").and_then(Value::as_object_mut){
        if let (Some(table),Some(string))=(pointer.get("tableID").and_then(|v|locale_id(v,true)),pointer.get("stringID").and_then(|v|locale_id(v,false))){pointer.insert("tableID".into(),json!(format!("0x{table:08x}")));pointer.insert("stringID".into(),json!(format!("0x{string:08x}")));}
    }
    if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{normalize_text_pointers(child)?}}Ok(())
}
/// @summary Read only supported image TGIs; an absent optional static preview remains explicitly unavailable.
fn image_reference(session:&ProviderSession,key:Key,diagnostics:&mut Vec<Value>)->Option<String>{
    let id=ResourceId{type_id:key.type_id,group:key.group,instance:key.instance};
    if matches!(id.type_id,0x2F7D0004|0x3F8662EA|0x2F7D0007)&&session.index.contains_key(&id){Some(typed(id))}else{diagnostics.push(json!({"code":"static_image_unavailable","reference":typed(id),"runtimeSubstitution":false}));None}
}
/// @summary Collect original image filenames, CSS/text classes and drawable types required by the actual tree.
fn collect_tree(session:&ProviderSession,node:&mut Value,assets:&mut BTreeMap<String,String>,styles:&mut BTreeSet<String>,counts:&mut BTreeMap<String,usize>,diagnostics:&mut Vec<Value>,parent_visible:bool)->Result<(),CommandError>{
    let visible=parent_visible&&node.get("visibility").and_then(Value::as_bool).unwrap_or(true);let mut unavailable=Vec::new();
    if let Some(drawable)=node.get("drawable").and_then(Value::as_object){
        if let Some(kind)=drawable.get("type").and_then(Value::as_u64){*counts.entry(kind.to_string()).or_default()+=1;}
        for field in ["cssStyles","nineSliceStyles","styles"]{if let Some(array)=drawable.get(field).and_then(Value::as_array){for style in array.iter().filter_map(Value::as_str){if !style.is_empty()&&!style.eq_ignore_ascii_case("none"){styles.insert(style.into());}}}}
        if let Some(images)=drawable.get("images").and_then(Value::as_array){for (state,reference) in images.iter().enumerate().filter_map(|(i,v)|v.as_str().map(|s|(i,s))){
            if reference.is_empty()||reference.eq_ignore_ascii_case("none"){continue}let name=reference.rsplit(['/', '\\']).next().unwrap_or(reference);let (stem,extension)=name.rsplit_once('.').ok_or_else(||fail(format!("Unsupported original image reference: {reference}")))?;
            let image_type=match extension.to_ascii_lowercase().as_str(){"png"=>0x2F7D0004,"jpg"|"jpeg"=>0x3F8662EA,"gif"=>0x2F7D0007,_=>return Err(fail(format!("Unsupported original image format: {reference}")))};
            let id=ResourceId{type_id:image_type,group:0,instance:fnv(stem)};if !session.index.contains_key(&id){
                let has_children=node.get("children").and_then(Value::as_array).is_some_and(|children|!children.is_empty());
                if state==0&&visible&&!has_children{return Err(fail(format!("Visible image-only control requires unavailable original image: {reference} ({id}), node={}",node.get("instanceID").unwrap_or(&Value::Null))))}
                unavailable.push(reference.to_owned());diagnostics.push(json!({"code":"original_layout_image_unavailable","reference":reference,"expectedResource":typed(id),"instanceID":node.get("instanceID"),"state":state,"visible":visible,"providerSubstitution":false,"rendererStateFallback":if state>0{"normal-if-requested-state-unavailable"}else{"none-for-missing-normal"},"renderingDegraded":true,"severity":if state==0&&visible{"error"}else{"warning"}}));continue
            }assets.insert(name.into(),typed(id));
        }}
    }
    if !unavailable.is_empty(){node["_unavailableResources"]=json!(unavailable);}
    if let Some(style)=node.get("textStyle").and_then(Value::as_str).filter(|s|!s.is_empty()){styles.insert(style.into());}
    if let Some(children)=node.get_mut("children").and_then(Value::as_array_mut){for child in children{collect_tree(session,child,assets,styles,counts,diagnostics,visible)?}}Ok(())
}
/// @summary Discover original stylesheet identities; headers supply display names only, never resource identity.
fn stylesheets(session:&ProviderSession)->Result<Vec<Value>,CommandError>{
    let mut ids:Vec<_>=session.index.keys().filter(|id|id.type_id==0x2C978DB6).copied().collect();ids.sort_by_key(|id|(id.group,id.instance));let mut result=Vec::new();
    for id in ids{let bytes=read(session,id)?;let text=std::str::from_utf8(&bytes).map_err(err)?;let header=&text[..text.char_indices().nth(512).map(|(i,_)|i).unwrap_or(text.len())];
        let tokens:Vec<_>=header.split(|c:char|c.is_whitespace()||matches!(c,'*'|'/'|'\r'|'\n'|'{'|'}')).collect();
        let name=tokens.iter().find(|w|w.to_ascii_lowercase().ends_with(".css")).map(|s|(*s).to_owned()).or_else(||tokens.iter().find(|w|w.starts_with(".autogen")&&w.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'.')).map(|s|format!("{}.css",s.trim_start_matches('.')))).unwrap_or_else(||format!("{:08X}.css",id.instance));
        result.push(json!({"name":name,"reference":typed(id)}));
    }if result.is_empty(){return Err(fail("No original installed stylesheets found"))}Ok(result)
}

/// @summary Assemble all non-debug declared categories and their static tools from original installed schemas.
/// @param session Verified read-only installation indexes.
/// @param locale Explicit installed locale identifier.
/// @param generation Exact installation generation pinned by the caller.
pub(crate) fn assemble(session:&ProviderSession,locale:&str,generation:&str)->Result<Value,CommandError>{
    let tables=locales(session,locale)?;let files=properties(session)?;let root_key=Key{type_id:PROP,group:0x09A4BA02,instance:0x57572DAC};let (_,palette_root)=property(&files,root_key)?;
    let category_keys=keys(palette_root,CATEGORY_IDS);let debug=values(palette_root,CATEGORY_DEBUG);
    if category_keys.is_empty()||debug.len()!=category_keys.len(){return Err(fail("Installed category palette schema is incomplete"))}
    let root_id=ResourceId{type_id:LAYOUT,group:LAYOUT_GROUP,instance:0x90D2CCCC};
    let (projected,editor_projection)=project_editor_hud(original_json(session,root_id)?)?;
    let mut count=0;let mut layout=inline(session,projected,&mut vec![root_id],&mut count)?;layout["sourceResource"]=json!(typed(root_id));
    namespace_modules(&mut layout,false);
    normalize_text_pointers(&mut layout)?;
    let mut categories=Vec::new();let mut tools=BTreeMap::new();let mut palettes=BTreeMap::new();let mut diagnostics=Vec::new();
    let mut assets=BTreeMap::new();let mut required_styles=BTreeSet::new();let mut drawable_counts=BTreeMap::new();
    // @summary These values retain the upstream editor geometry, not a claim about live controller state.
    let slot_row=json!({"pitch":121.6,"startX":245,"slotW":121.6,"slotH":50,"topInBar":4});
    for category_id in EDITOR_CATEGORY_IDS{
        let ordinal=category_keys.iter().position(|key|key.instance==*category_id).ok_or_else(||fail(format!("Required editor category {category_id:08X} is missing")))?;
        let key=&category_keys[ordinal];
        let is_debug=match debug[ordinal]{PropValue::Bool(value)=>*value,_=>return Err(fail("Category debug flag is not boolean"))};if is_debug{continue}
        let (category_resource,_)=property(&files,*key)?;let category=inherited(&files,*key,&mut Vec::new())?;
        let title=texts(&category,CATEGORY_NAME).first().copied().ok_or_else(||fail(format!("Category {:08X} has no localized name reference",key.instance)))?;
        let label=localized(&tables,title)?;let icon=keys(&category,CATEGORY_ICONS).first().copied().and_then(|k|image_reference(session,k,&mut diagnostics));
        let palette_key=keys(&category,CATEGORY_LAYOUT).first().copied().ok_or_else(||fail(format!("Category {:08X} has no palette layout",key.instance)))?;
        let palette_id=ResourceId{type_id:palette_key.type_id,group:palette_key.group,instance:palette_key.instance};
        if palette_id.type_id!=LAYOUT||palette_id.group!=LAYOUT_GROUP{return Err(fail("Category references unsupported non-declarative palette"))}
        let mut palette=original_json(session,palette_id)?;project_palette_controls(&mut palette,&mut Vec::new(),key.instance)?;let mut count=0;let mut palette=inline(session,palette,&mut vec![palette_id],&mut count)?;palette["sourceResource"]=json!(typed(palette_id));namespace_modules(&mut palette,true);normalize_text_pointers(&mut palette)?;collect_tree(session,&mut palette,&mut assets,&mut required_styles,&mut drawable_counts,&mut diagnostics,true)?;
        let category_id=format!("{:08X}",key.instance);categories.push(json!({"id":category_id,"label":label,"icon":icon,"iconHash":keys(&category,CATEGORY_ICONS).first().map(|k|format!("{:08X}",k.instance))}));
        palettes.insert(category_id.clone(),json!({"layoutInstance":format!("{:08X}",palette_id.instance),"layout":palette,"slotRow":slot_row}));
        let listed=keys(&category,TOOL_LIST);let mut candidate_ids=Vec::new();
        for (id,file) in &files{if !matches!(id.group&0xFFFF,0x8A01|0xC900){continue}let mut membership=keys(file,PARENT);membership.extend(keys(file,UI_CATEGORY));if references(session,&listed,*id)?||references(session,&membership,category_resource)?{candidate_ids.push(*id)}}
        candidate_ids.sort_by_key(|id|(files.get(id).and_then(|f|number(f,UI_POSITION)).unwrap_or(i64::MAX),id.instance));
        let mut entries=Vec::new();
        for id in candidate_ids{
            let file=inherited(&files,Key{type_id:PROP,group:id.group,instance:id.instance},&mut Vec::new())?;
            let label=editor_tool_text(&tables,&file,TITLE,0x6C969DEE)?;
            // @summary Preserve the upstream editor's supported statistic references; later-game additions were absent from that authoring corpus.
            let stats:Vec<_>=texts(&file,EFFECT_TITLE).into_iter().filter(|t|is_editor_statistic(*t)).map(|t|localized(&tables,t).map(|label|static_statistic(&label))).collect::<Result<_,_>>()?;
            let preview=keys(&file,TOOL_ICON).first().copied().and_then(|k|image_reference(session,k,&mut diagnostics)).or_else(||(category_id=="3193C0BE").then(||typed(ResourceId{type_id:0x2F7D0004,group:0x40E02400,instance:0xF71F4A5F})));let marquee=keys(&file,MARQUEE).first().copied().and_then(|k|image_reference(session,k,&mut diagnostics));
            entries.push(json!({"instance":format!("{:08X}",id.instance),"label":label,"desc":editor_tool_text(&tables,&file,DESCRIPTION,0x50AA0BEA)?,"unlock":editor_tool_text(&tables,&file,UNLOCK,0x4B54417A)?,"stats":stats,"cost":null,"upkeep":null,"preview":preview,"marquee":marquee,"locked":file.get(HARD_GATE).is_some(),"unlockState":"unknown","staticRequirements":{"hasHardGateProperty":file.get(HARD_GATE).is_some(),"lockedBadgeSemantics":"upstream-editor-static-requirement-not-live-state"},"sourceResource":typed(id)}));
        }
        if category_id=="899BEA70"&&entries.is_empty(){for instance in EDITOR_PARK_IDS{let file=inherited(&files,Key{type_id:PROP,group:0x09878A01,instance:*instance},&mut Vec::new())?;let title=texts(&file,TITLE).first().copied().ok_or_else(||fail(format!("Required park {instance:08X} has no original title")))?;entries.push(json!({"instance":format!("{instance:08X}"),"label":localized(&tables,title)?,"preview":null,"marquee":null,"locked":false,"sourceResource":typed(ResourceId{type_id:PROP,group:0x09878A01,instance:*instance})}));}}
        tools.insert(category_id,entries);
    }
    if categories.is_empty(){return Err(fail("Installed palette declares no non-debug categories"))}
    collect_tree(session,&mut layout,&mut assets,&mut required_styles,&mut drawable_counts,&mut diagnostics,true)?;
    for (name,group,instance) in [("tool_placeholder.png",0x40E02400,0xF71F4A5F),("icn-simoleon.png",0,0x3644CB0C),("palette-tool-select-hl.png",0,0x05326CB3)]{let id=ResourceId{type_id:0x2F7D0004,group,instance};if !session.index.contains_key(&id){return Err(fail(format!("Required authoring image is missing: {name} ({id})")))}assets.insert(name.into(),typed(id));}
    let locale_json:BTreeMap<_,_>=tables.iter().map(|(table,items)|(format!("0x{table:08x}"),items.iter().map(|(id,text)|(format!("0x{id:08x}"),text.clone())).collect::<BTreeMap<_,_>>())).collect();
    diagnostics.push(json!({"code":"static_authoring_preview","runtimeSimulationValuesAvailable":false,"runtimeUnlockStateAvailable":false,"geometry":"upstream authoring calibration retained: category pitch 46.85, size 43, center 875.5; palette pitch 121.6, start 245, height 50; not live controller state"}));
    diagnostics.push(json!({"code":"editor_scene_projection","scene":"city-hud-authoring","verifiedRuntimeState":false,"retainedSharedControlIds":EDITOR_HUD_CONTROLS}));
    Ok(json!({"previewGeneration":generation,"layout":layout,"assets":assets,"locale":locale_json,"resolvedLocale":locale,"model":{"categories":categories,"row":{"pitch":46.85,"size":43,"centerX":875.5,"offsetY":0}},"tools":tools,"palette":{"byCategory":palettes,"row":slot_row},"requiredStyleClasses":required_styles,"originalDrawableTypeCounts":drawable_counts,"stylesheets":stylesheets(session)?,"diagnostics":diagnostics,"editorProjection":editor_projection,"source":"installed-game-direct"}))
}

#[cfg(test)]mod tests{
    use super::*;
    /// @summary Validate upstream authoring contracts against an explicitly selected installed game without exporting payloads.
    #[test] #[ignore="requires explicit OPEN_SCP_GAME_DATA for the supported game installation"]
    fn installed_upstream_authoring_contract(){
        /// @summary Collect package paths only inside the explicitly selected test installation.
        fn scan(path:&std::path::Path,paths:&mut Vec<std::path::PathBuf>){for entry in std::fs::read_dir(path).unwrap(){let path=entry.unwrap().path();if path.is_dir(){scan(&path,paths)}else if path.extension().is_some_and(|v|v.eq_ignore_ascii_case("package")){paths.push(path)}}}
        let mut paths=Vec::new();scan(std::path::Path::new(&std::env::var("OPEN_SCP_GAME_DATA").unwrap()),&mut paths);paths.sort_by_key(|path|{let name=path.file_name().unwrap().to_string_lossy().to_ascii_lowercase();let rank=if name.contains("ep1"){4}else if name.contains("dlc"){3}else if name.contains("graphics"){2}else if name.contains("game"){1}else{0};(rank,path.to_string_lossy().to_ascii_lowercase())});
        let mut session=ProviderSession::default();for path in paths{let package=dbpf::Package::open(path).unwrap();let pi=session.packages.len();for (ei,entry) in package.entries().iter().enumerate(){session.index.insert(entry.id,(pi,ei));}session.packages.push(package)}
        let locales=available_locales(&session);let locale=locales.iter().find(|locale|locale.as_str()=="zh-TW").cloned().unwrap_or_else(||locales.into_iter().next().unwrap());let model=assemble(&session,&locale,"test").unwrap();
        // @summary Layout text remains original inert textContent input, including literal markup in the upstream date template.
        let source_locales=super::locales(&session,&locale).unwrap();let date=&source_locales[&0x2EDD95CA][&0x0BCFD595];if locale=="zh-TW"{assert!(date.contains("<br>"));}assert_eq!(model["locale"]["0x2edd95ca"]["0x0bcfd595"],json!(date));
        /// @summary Count actual expanded authoring controls for the upstream scene contract.
        fn count(node:&Value)->usize{1+node.get("children").and_then(Value::as_array).map(|children|children.iter().map(count).sum::<usize>()).unwrap_or_default()}
        assert_eq!(count(&model["layout"]),231);assert_eq!(model["model"]["categories"].as_array().unwrap().len(),14);assert_eq!(model["tools"]["899BEA70"].as_array().unwrap().len(),56);
        let tools:Vec<_>=model["tools"].as_object().unwrap().values().flat_map(|value|value.as_array().unwrap()).collect();assert_eq!(tools.len(),112);assert_eq!(tools.iter().filter(|tool|tool.get("locked")==Some(&Value::Bool(true))).count(),45);
        let stats:Vec<_>=tools.iter().flat_map(|tool|tool.get("stats").and_then(Value::as_array).into_iter().flatten()).collect();assert_eq!(stats.len(),82);assert_eq!(stats.iter().filter(|stat|!stat["value"].is_null()).count(),21);
        assert_eq!(model["model"]["row"],json!({"pitch":46.85,"size":43,"centerX":875.5,"offsetY":0}));assert_eq!(model["palette"]["row"],json!({"pitch":121.6,"startX":245,"slotW":121.6,"slotH":50,"topInBar":4}));
        for (category,expected) in EDITOR_CATEGORY_IDS.iter().zip([44,56,32,32,32,33,32,32,32,32,32,30,30,18]){assert_eq!(count(&model["palette"]["byCategory"][format!("{category:08X}")]["layout"]),expected,"palette {category:08X}");}
        if let Ok(path)=std::env::var("OPEN_SCP_BASELINE_TOOLS"){
            assert_eq!(locale,"zh-TW","The original reference corpus requires the same installed language");
            let source=std::fs::read_to_string(path).unwrap();let json=source.split_once('=').unwrap().1.trim().trim_end_matches(';');let baseline:Value=serde_json::from_str(json).unwrap();
            for (category,entries) in baseline.as_object().unwrap(){for (index,expected) in entries.as_array().unwrap().iter().enumerate(){let actual=&model["tools"][category][index];for field in ["instance","label","desc","unlock","stats","cost","upkeep","locked"]{assert_eq!(actual[field],expected[field],"tool {category}[{index}].{field}");}}}
        }
    }
    /// @summary Wrapper identity and explicit properties override a module while children remain declarative.
    #[test]fn module_overrides_preserve_children(){let template=json!({"width":30,"children":[{"instanceID":1}]});let wrapper=json!({"instanceID":9,"width":60,"children":[{"instanceID":2}]});let result=merge_module(template,wrapper.as_object().unwrap(),"Layouts/Example.js").unwrap();assert_eq!(result["width"],60);assert_eq!(result["instanceID"],9);assert_eq!(result["children"].as_array().unwrap().len(),2);assert_eq!(result["_module"],"Layouts/Example.js");}
    /// @summary Nominal JavaScript filenames resolve only to the known declarative layout type/group.
    #[test]fn layout_names_never_execute_scripts(){assert_eq!(layout_id("Layouts/GlobalUI/globalui2.js").unwrap().instance,0x90D2CCCC);assert!(layout_id("https://evil/x.js").is_err());assert!(layout_id("Layouts/not-json.html").is_err());assert!(original_json(&ProviderSession::default(),layout_id("example.js").unwrap()).is_err());}
    /// @summary Locale source names and numeric IDs normalize to one exact table identity.
    #[test]fn locale_pointer_identity(){assert_eq!(locale_id(&json!("GameEntry.json"),true),Some(fnv("gameentry")));assert_eq!(locale_id(&json!("0x12345678"),false),Some(0x12345678));assert!(locale_id(&json!("garbage"),false).is_none());}
    /// @summary Only literal installed statistic values are displayed, retaining unknown runtime values and inert labels.
    #[test]fn statistics_preserve_static_values_without_html(){assert_eq!(static_statistic("<B>Lanes</B>: 4"),json!({"label":"Lanes","value":"4"}));assert_eq!(static_statistic("<B>Rate</B>: ~amount:number~ / h"),json!({"label":"Rate","value":null}));assert_eq!(static_statistic("<B>Effect</B>"),json!({"label":"Effect","value":null}));assert_eq!(static_statistic("<B> Value</B>： literal"),json!({"label":" Value","value":"literal"}));assert_eq!(static_statistic("<B>Rate</B>：~amount:number~ / h"),json!({"label":"Rate","value":null}));}
    /// @summary Multi-condition unlock arrays and unsupported DLC tables keep the original editor's empty-field behavior.
    #[test]fn tool_text_projection_preserves_supported_reference_shapes(){let reference=Text{table_id:0x4B54417A,instance_id:1};assert_eq!(editor_text_reference(&[reference],0x4B54417A),Some(reference));assert!(editor_text_reference(&[reference,reference],0x4B54417A).is_none());assert!(editor_text_reference(&[reference],0x6C969DEE).is_none());assert!(editor_text_reference(&[],0x4B54417A).is_none());}
    /// @summary An unrelated locale table cannot lose a statistic because its instance number collides with an excluded reference.
    #[test]fn statistic_projection_preserves_table_identity(){assert!(!is_editor_statistic(Text{table_id:0xFCFEA360,instance_id:0x0FFBA772}));assert!(!is_editor_statistic(Text{table_id:0x2FAE9986,instance_id:0x0EF3E7F5}));assert!(is_editor_statistic(Text{table_id:0x50AA0BEA,instance_id:0x0FFBA772}));assert!(is_editor_statistic(Text{table_id:0x50AA0BEA,instance_id:0x0E684F0F}));}
    /// @summary The editor resolves verified module variants rather than similarly named incompatible templates.
    #[test]fn compatibility_modules_use_verified_resources(){assert_eq!(layout_id("Layouts/Palette/CategoryButtonWithIcon2.js").unwrap().instance,0x33B37BAE);assert_eq!(layout_id("Layouts/Palette/BudgetButton.js").unwrap().instance,0xDE286E0E);assert_eq!(layout_id("Layouts/Palette/SpeedPanel.js").unwrap().instance,0x154B8B98);assert_eq!(EDITOR_CATEGORY_IDS.len(),14);assert_eq!(EDITOR_PARK_IDS.len(),56);}
    /// @summary Existing editor override identities survive both HUD and palette module expansion.
    #[test]fn module_namespaces_preserve_editor_references(){let source=json!({"instanceID":22,"_module":"Layouts/GlobalUI/CloseButton.js","children":[{"instanceID":2,"children":[{"instanceID":3}]}]});let mut hud=source.clone();namespace_modules(&mut hud,false);assert_eq!(hud["children"][0]["instanceID"],"22::2");assert_eq!(hud["children"][0]["children"][0]["instanceID"],"22::3");let mut palette=source;namespace_modules(&mut palette,true);assert_eq!(palette["children"][0]["instanceID"],"22|CloseButton.js::2");}
    /// @summary Missing hidden images remain explicit unresolved metadata, never fabricated image identities.
    #[test]fn missing_images_are_diagnosed_without_substitution(){let mut node=json!({"instanceID":1,"visibility":false,"drawable":{"type":2,"images":["missing.png"]}});let mut assets=BTreeMap::new();let mut styles=BTreeSet::new();let mut counts=BTreeMap::new();let mut diagnostics=Vec::new();collect_tree(&ProviderSession::default(),&mut node,&mut assets,&mut styles,&mut counts,&mut diagnostics,true).unwrap();assert!(assets.is_empty());assert_eq!(node["_unavailableResources"][0],"missing.png");assert_eq!(diagnostics[0]["providerSubstitution"],false);node["visibility"]=json!(true);assert!(collect_tree(&ProviderSession::default(),&mut node,&mut assets,&mut styles,&mut counts,&mut diagnostics,true).is_err());}
    /// @summary Native menu family context excludes unrelated instances but never hides a same-family collision.
    #[test]fn zero_group_membership_uses_typed_family(){let mut session=ProviderSession::default();let target=ResourceId{type_id:PROP,group:0x09A4BA03,instance:1};session.index.insert(target,(0,0));session.index.insert(ResourceId{type_id:PROP,group:0x40E0EC00,instance:1},(0,1));let generic=Key{type_id:0,group:0,instance:1};assert!(references(&session,&[generic],target).unwrap());session.index.insert(ResourceId{type_id:PROP,group:0x0E0EBA03,instance:1},(0,2));assert!(references(&session,&[generic],target).is_err());assert!(references(&session,&[Key{type_id:PROP,group:0x09A4BA03,instance:1}],target).unwrap());assert!(!references(&session,&[Key{type_id:PROP,group:0x40E0EC00,instance:1}],target).unwrap());}
    /// @summary A declared editor projection excludes runtime dialogs without changing retained geometry or source visibility.
    #[test]fn hud_projection_is_explicit_and_preserves_native_nodes(){let controls:Vec<_>=EDITOR_HUD_CONTROLS.iter().map(|id|json!({"instanceID":id,"left":17,"top":23,"width":99,"visibility":true,"children":[]})).chain(std::iter::once(json!({"instanceID":1334,"comment":"MAIN MENU","visibility":true}))).collect();let source=json!({"instanceID":1,"width":1024,"children":[{"instanceID":795,"children":controls},{"instanceID":977,"comment":"LoadingScreen","visibility":true}]});let (projected,metadata)=project_editor_hud(source).unwrap();assert_eq!(projected["children"].as_array().unwrap().len(),1);let retained=projected["children"][0]["children"].as_array().unwrap();assert_eq!(retained.len(),EDITOR_HUD_CONTROLS.len());assert_eq!(retained[0]["left"],17);assert_eq!(retained[0]["visibility"],true);assert_eq!(metadata["verifiedRuntimeState"],false);assert_eq!(metadata["excludedRoots"][0]["instanceID"],977);assert_eq!(metadata["excludedSharedControls"][0]["instanceID"],1334);assert!(project_editor_hud(json!({"children":[]})).is_err());}
}
