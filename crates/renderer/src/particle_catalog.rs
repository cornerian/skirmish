//! Source aliases for visually classified particle texture appearances.
//! No decoded images, emitter bytecode or original binary data are embedded.
use crate::particles::ParticleEffect;
use clap::ValueEnum;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct ParticleTextureRecipe {
    pub frame_sha256: String,
    /// Visual classification, not an original gameplay effect name.
    pub visual_family: String,
    pub reference_size: [u32; 2],
    pub aliases: Vec<ParticleTextureAlias>,
}

#[derive(Debug, Deserialize)]
pub struct ParticleTextureAlias {
    pub source: String,
    pub source_sha256: String,
    pub descriptor_offset: u32,
    pub image_offset: u32,
}

impl ParticleTextureRecipe {
    /// Unknown families return None rather than silently using a generic puff.
    pub fn effect(&self) -> Option<ParticleEffect> {
        ParticleEffect::from_str(&self.visual_family.replace('_', "-"), false).ok()
    }
}

pub fn texture_recipes() -> &'static [ParticleTextureRecipe] {
    static CATALOG: OnceLock<Vec<ParticleTextureRecipe>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        include_str!("../particle_sources.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).expect("validated particle source recipe"))
            .collect()
    })
}

/// Resolve once when loading native materials; keep the resulting effect enum
/// on the emitter/material instead of looking up strings for each particle.
pub fn effect_for_texture(rgba_sha256: &str) -> Option<ParticleEffect> {
    texture_recipes()
        .iter()
        .find(|r| r.frame_sha256 == rgba_sha256)?
        .effect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_aliases_are_complete_unique_and_resolvable_for_enabled_families() {
        let recipes = texture_recipes();
        assert_eq!(recipes.len(), 337);
        assert_eq!(recipes.iter().map(|r| r.aliases.len()).sum::<usize>(), 593);
        let mut hashes = std::collections::HashSet::new();
        for recipe in recipes {
            assert!(hashes.insert(&recipe.frame_sha256));
            assert_eq!(recipe.frame_sha256.len(), 64);
            assert!(recipe.reference_size.iter().all(|&n| n > 0));
            assert!(!recipe.aliases.is_empty());
            assert!(
                recipe.effect().is_some(),
                "unimplemented family {}",
                recipe.visual_family
            );
            for alias in &recipe.aliases {
                assert_eq!(alias.source_sha256.len(), 64);
                assert!(alias.source.starts_with("files/"));
            }
        }
        for effect in ParticleEffect::value_variants() {
            assert!(
                recipes.iter().any(|r| r.effect() == Some(*effect)),
                "missing source for {effect:?}"
            );
        }
        assert!(effect_for_texture("unknown").is_none());
        assert_eq!(
            effect_for_texture("f43fd5bcaff5ddb9bc5647a73fc76dca39bc4eee4db1e89c6b51233d36aae80f"),
            Some(ParticleEffect::Smoke)
        );
    }
}
