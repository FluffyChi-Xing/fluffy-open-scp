//! Build-time subset of Simoder's owner/generation + view/slot composition.
//! Source: Simoder ui_patch.rs register_patch/revoke_generation/compose_sorted.
//! Unlike a live DOM registry this composes our existing, tested ES5 adapters.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Patch {
    pub id: String,
    pub owner: String,
    pub generation: u64,
    pub view: String,
    pub slot: String,
    pub priority: i32,
    pub requires: Vec<String>,
    pub source: String,
}

#[derive(Default)]
pub(super) struct Registry { patches: BTreeMap<String, Patch> }
impl Registry {
    pub fn register(&mut self, patch: Patch) -> Result<(), String> {
        if patch.owner.is_empty() || patch.generation == 0 || patch.view.is_empty()
            || patch.slot.is_empty() || !patch.id.starts_with(&format!("{}.",patch.owner)) {
            return Err("PANEL_RUNTIME_PATCH_IDENTITY".into());
        }
        if self.patches.contains_key(&patch.id) {
            return Err(format!("PANEL_RUNTIME_COMPOSITION_CONFLICT: duplicate {}",patch.id));
        }
        self.patches.insert(patch.id.clone(),patch);
        Ok(())
    }
    pub fn revoke_generation(&mut self, owner: &str, generation: u64) -> usize {
        let before=self.patches.len();
        self.patches.retain(|_,p|p.owner!=owner || p.generation!=generation);
        before-self.patches.len()
    }
    pub fn compose(&self) -> Result<Vec<&Patch>, String> {
        let mut ordered:Vec<_>=self.patches.values().collect();
        ordered.sort_by_key(|p|(p.priority,&p.owner,&p.id));
        let mut slots=BTreeMap::new();
        let mut available=BTreeSet::new();
        for p in &ordered {
            if let Some(previous)=slots.insert((&p.view,&p.slot),&p.id) {
                return Err(format!("PANEL_RUNTIME_COMPOSITION_CONFLICT: {}/{}: {previous}, {}",p.view,p.slot,p.id));
            }
            for required in &p.requires {
                if !available.contains(required) {
                    return Err(format!("PANEL_RUNTIME_PATCH_DEPENDENCY: {} requires {required}",p.id));
                }
            }
            available.insert(p.id.clone());
        }
        Ok(ordered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patch(owner:&str,slot:&str,priority:i32)->Patch {
        Patch{id:format!("{owner}.{slot}"),owner:owner.into(),generation:1,
            view:"building-details".into(),slot:slot.into(),priority,requires:vec![],source:owner.into()}
    }
    #[test]
    fn deterministic_composition_rejects_competing_slots_and_revokes_only_owned_generation() {
        let mut registry=Registry::default();
        registry.register(patch("waco","office",20)).unwrap();
        registry.register(patch("peace","residence",10)).unwrap();
        assert_eq!(registry.compose().unwrap().iter().map(|p|p.owner.as_str()).collect::<Vec<_>>(),["peace","waco"]);
        registry.register(patch("third","office",30)).unwrap();
        assert!(registry.compose().unwrap_err().contains("waco.office"));
        assert_eq!(registry.revoke_generation("third",2),0);
        assert_eq!(registry.revoke_generation("third",1),1);
        assert_eq!(registry.compose().unwrap().len(),2);
        let mut dependent=patch("consumer","extra",1);
        dependent.requires=vec!["waco.office".into()];
        registry.register(dependent).unwrap();
        assert!(registry.compose().unwrap_err().contains("PATCH_DEPENDENCY"));
    }
}
