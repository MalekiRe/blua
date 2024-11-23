use bevy::app::App;
use bevy::asset::{AssetServer, Handle};
use bevy::prelude::ReflectComponent;
use bevy::prelude::*;
use bevy::prelude::{Commands, Component, Reflect, Res, Startup, Transform, Vec3};
use bevy::reflect::func::{ArgList, Return};
use bevy::DefaultPlugins;
use blua::asset_loader::LuaScript;
use blua::{AppExtensionFunctionRegisterTrait, BluaScript, LuaPlugin};
use std::any::{Any, TypeId};
use std::ops::Add;

fn main() {
    let mut app = App::default();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .add_plugins(LuaPlugin);
    app.register_type::<CubeMarker>();
    app.register_object_function::<Vec3>(add.into_function().with_name("add"));
    app.register_non_self_object_function::<Transform>(
        default_uwu.into_function().with_name("default"),
    );
    app.register_type::<Transform>();
    app.add_systems(Startup, setup);
    app.run();
}

fn add(a: &Vec3, b: &Vec3) -> Vec3 {
    a + b
}

fn default_uwu() -> Transform {
    Transform::default()
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct CubeMarker {
    a: f32,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    /* commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 0.0),
        CubeMarker {
            a: 10.0
        },
    ));
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));*/

    commands.spawn(BluaScript(asset_server.load("cube.lua")));
}

#[derive(Component)]
pub struct HandleHolder {
    handle: Handle<LuaScript>,
}
