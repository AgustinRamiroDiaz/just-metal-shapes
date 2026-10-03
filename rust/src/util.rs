//! Small Godot interop helpers.

use godot::prelude::*;

/// `dict[key] = value` for an untyped dictionary.
pub fn dict_set(dict: &mut VarDictionary, key: &str, value: impl ToGodot) {
    dict.set(&key.to_variant(), &value.to_variant());
}
