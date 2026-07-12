use lumina_schema::{Object, Scene};
use std::collections::HashMap;

/// The scene's object map plus the set of root objects (those not claimed
/// as a child by any group).
pub struct SceneGraph {
    /// Every object in the scene, keyed by id.
    pub objects: HashMap<String, Object>,
    /// Ids of objects drawn at the root (groups draw their own children).
    pub root_objects: Vec<String>,
}

impl SceneGraph {
    /// Build the graph from a parsed scene, resolving group membership.
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

    /// Insert (or replace) an object and refresh root membership.
    pub fn add_object(&mut self, id: String, obj: Object) {
        self.objects.insert(id, obj);
    }

    /// Remove an object and drop it from the root list.
    pub fn remove_object(&mut self, id: &str) {
        self.objects.remove(id);
        // Also remove from any groups
        for obj in self.objects.values_mut() {
            if let Object::Group(group) = obj {
                group.children.retain(|c| c != id);
            }
        }
    }

    /// The object stored under `id`, if any.
    pub fn get_object(&self, id: &str) -> Option<&Object> {
        self.objects.get(id)
    }
}
