use std::collections::HashMap;

use smithay::utils::Transform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl OutputGeometry {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let left = self.x as f64;
        let top = self.y as f64;
        let right = left + self.width as f64;
        let bottom = top + self.height as f64;
        x >= left && x < right && y >= top && y < bottom
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputModeInfo {
    pub width: i32,
    pub height: i32,
    pub refresh_millihz: Option<i32>,
    pub current: bool,
    pub preferred: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub id: OutputId,
    pub name: String,
    pub geometry: OutputGeometry,
    pub scale: f64,
    pub transform: Transform,
    pub refresh_millihz: Option<i32>,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputRegistration {
    pub name: String,
    pub geometry: OutputGeometry,
    pub scale: f64,
    pub transform: Transform,
    pub refresh_millihz: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputReconfigure {
    pub geometry: OutputGeometry,
    pub scale: f64,
    pub transform: Transform,
    pub refresh_millihz: Option<i32>,
    pub primary: Option<bool>,
}

#[derive(Debug, Default)]
pub struct OutputRegistry {
    next_id: u32,
    outputs: Vec<OutputInfo>,
    modes: HashMap<OutputId, Vec<OutputModeInfo>>,
}

impl OutputRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> &[OutputInfo] {
        &self.outputs
    }

    pub fn modes_for_id(&self, id: OutputId) -> &[OutputModeInfo] {
        self.modes.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn set_modes_by_id(&mut self, id: OutputId, modes: Vec<OutputModeInfo>) -> bool {
        if !self.contains_id(id) {
            return false;
        }
        self.modes.insert(id, modes);
        true
    }

    pub fn set_modes_by_name(
        &mut self,
        name: &str,
        modes: Vec<OutputModeInfo>,
    ) -> Option<OutputId> {
        let id = self.by_name(name)?.id;
        self.set_modes_by_id(id, modes).then_some(id)
    }

    pub fn first(&self) -> Option<&OutputInfo> {
        self.outputs.first()
    }

    pub fn primary(&self) -> Option<&OutputInfo> {
        self.outputs
            .iter()
            .find(|output| output.primary)
            .or_else(|| self.first())
    }

    pub fn by_id(&self, id: OutputId) -> Option<&OutputInfo> {
        self.outputs.iter().find(|output| output.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&OutputInfo> {
        self.outputs.iter().find(|output| output.name == name)
    }

    pub fn contains_id(&self, id: OutputId) -> bool {
        self.by_id(id).is_some()
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.outputs.iter().any(|output| output.name == name)
    }

    pub fn output_at_point(&self, x: f64, y: f64) -> Option<&OutputInfo> {
        self.outputs
            .iter()
            .find(|output| output.geometry.contains(x, y))
    }

    pub fn select_for_point_with_fallback(&self, x: f64, y: f64) -> Option<&OutputInfo> {
        self.output_at_point(x, y).or_else(|| self.primary())
    }

    pub fn upsert(&mut self, registration: OutputRegistration) -> OutputId {
        if let Some(existing) = self
            .outputs
            .iter_mut()
            .find(|output| output.name == registration.name)
        {
            existing.geometry = registration.geometry;
            existing.scale = registration.scale;
            existing.transform = registration.transform;
            existing.refresh_millihz = registration.refresh_millihz;
            return existing.id;
        }

        let id = OutputId(self.next_id.saturating_add(1));
        self.next_id = id.0;
        let primary = self.outputs.is_empty();
        self.outputs.push(OutputInfo {
            id,
            name: registration.name,
            geometry: registration.geometry,
            scale: registration.scale,
            transform: registration.transform,
            refresh_millihz: registration.refresh_millihz,
            primary,
        });
        id
    }

    fn ensure_primary_after_mutation(&mut self) {
        if self.outputs.is_empty() {
            return;
        }
        if self.outputs.iter().any(|output| output.primary) {
            return;
        }
        if let Some(first) = self.outputs.first_mut() {
            first.primary = true;
        }
    }

    pub fn remove_by_id(&mut self, id: OutputId) -> Option<OutputInfo> {
        let idx = self.outputs.iter().position(|output| output.id == id)?;
        let removed = self.outputs.remove(idx);
        self.modes.remove(&removed.id);
        self.ensure_primary_after_mutation();
        Some(removed)
    }

    pub fn remove_by_name(&mut self, name: &str) -> Option<OutputInfo> {
        let idx = self.outputs.iter().position(|output| output.name == name)?;
        let removed = self.outputs.remove(idx);
        self.modes.remove(&removed.id);
        self.ensure_primary_after_mutation();
        Some(removed)
    }

    pub fn reconfigure_by_id(&mut self, id: OutputId, reconfigure: OutputReconfigure) -> bool {
        let Some(existing) = self.outputs.iter_mut().find(|output| output.id == id) else {
            return false;
        };
        existing.geometry = reconfigure.geometry;
        existing.scale = reconfigure.scale;
        existing.transform = reconfigure.transform;
        existing.refresh_millihz = reconfigure.refresh_millihz;
        if let Some(primary) = reconfigure.primary {
            existing.primary = primary;
        }
        true
    }

    pub fn reconfigure_by_name(
        &mut self,
        name: &str,
        reconfigure: OutputReconfigure,
    ) -> Option<OutputId> {
        let id = self.outputs.iter().find(|output| output.name == name)?.id;
        if self.reconfigure_by_id(id, reconfigure) {
            return Some(id);
        }
        None
    }
}

#[cfg(test)]
#[path = "output_registry_tests.rs"]
mod tests;
