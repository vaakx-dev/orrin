mod common;

use common::{clean_text, merge};
use orrin_core::service::Merged;

const BASE: &str = r#"[gd_scene load_steps=2 format=3 uid="uid://main"]

[ext_resource type="Script" path="res://player.gd" id="1_a"]

[node name="Main" type="Node2D"]

[node name="Player" type="Sprite2D" parent="."]
position = Vector2(0, 0)
script = ExtResource("1_a")
"#;

#[test]
fn separate_sections_merge_and_recount_load_steps() {
    let ours = BASE.replace("load_steps=2", "load_steps=3").replace(
        "[node name=\"Main\"",
        "[ext_resource type=\"Texture2D\" path=\"res://enemy.png\" id=\"2_b\"]\n\n[node name=\"Main\"",
    ) + "\n[node name=\"Enemy\" type=\"Sprite2D\" parent=\".\"]\ntexture = ExtResource(\"2_b\")\n";
    let theirs = BASE.replace("Vector2(0, 0)", "Vector2(5, 5)")
        + "\n[sub_resource type=\"CircleShape2D\" id=\"Circle_c\"]\nradius = 4.0\n";
    let merged = clean_text(merge("main.tscn", BASE.as_bytes(), ours.as_bytes(), theirs.as_bytes()));
    assert!(merged.starts_with("[gd_scene load_steps=4 format=3"), "{merged}");
    assert!(merged.contains("position = Vector2(5, 5)"));
    assert!(merged.contains("[ext_resource type=\"Texture2D\" path=\"res://enemy.png\" id=\"2_b\"]"));
    assert!(merged.contains("[node name=\"Enemy\" type=\"Sprite2D\" parent=\".\"]\ntexture = ExtResource(\"2_b\")"));
    assert!(merged.contains("[sub_resource type=\"CircleShape2D\" id=\"Circle_c\"]\nradius = 4.0"));
}

#[test]
fn same_property_changed_twice_conflicts() {
    let ours = BASE.replace("Vector2(0, 0)", "Vector2(1, 1)");
    let theirs = BASE.replace("Vector2(0, 0)", "Vector2(2, 2)");
    let merged = merge("main.tscn", BASE.as_bytes(), ours.as_bytes(), theirs.as_bytes());
    assert!(matches!(merged, Merged::Conflict(reason) if reason.contains("Player")));
}
