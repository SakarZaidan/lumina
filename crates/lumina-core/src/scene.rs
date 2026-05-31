use lumina_schema::{Object, Scene};
use std::collections::HashMap;

pub struct SceneGraph {
    pub objects: HashMap<String, Object>,
    pub root_objects: Vec<String>,
}

impl SceneGraph {
    pub fn from_scene(scene: &Scene) -> Self {
        let objects = scene.objects.clone();

        // Find root objects (those not contained in any group)
        let mut child_ids = std::collections::HashSet::new();
        for obj in objects.values() {
            if let Object::Group(group) = obj {
                for child_id in &group.children {
                    child_ids.insert(child_id.clone());
                }
            }
        }

        let mut root_objects = Vec::new();
        for id in objects.keys() {
            if !child_ids.contains(id) {
                root_objects.push(id.clone());
            }
        }

        Self {
            objects,
            root_objects,
        }
    }

    pub fn add_object(&mut self, id: String, obj: Object) {
        self.objects.insert(id, obj);
    }

    pub fn remove_object(&mut self, id: &str) {
        self.objects.remove(id);
        // Also remove from any groups
        for obj in self.objects.values_mut() {
            if let Object::Group(group) = obj {
                group.children.retain(|c| c != id);
            }
        }
    }

    pub fn get_object(&self, id: &str) -> Option<&Object> {
        self.objects.get(id)
    }
}
