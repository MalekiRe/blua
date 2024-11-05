use bevy::app::App;
use bevy::asset::{AssetServer, Handle};
use bevy::prelude::ReflectComponent;
use bevy::prelude::*;
use bevy::prelude::{Commands, Component, Reflect, Res, Startup, Transform, Vec3};
use bevy::DefaultPlugins;
use blua::asset_loader::LuaScript;
use blua::LuaPlugin;
fn main() {
    let mut app = App::default();
    app.add_plugins(DefaultPlugins).add_plugins(LuaPlugin);
    app.add_systems(Startup, setup);
    app.register_type::<Stretch>();
    app.run();
}

#[derive(Component, Reflect, PartialEq, Debug, Default)]
#[reflect(Component, Default, PartialEq, Debug)]
pub struct Stretch {
    pub x: f32,
    pub y: f32,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Transform::from_translation(Vec3::new(3.0, 4.0, 6.0)),
        Stretch::default(),
    ));
    commands.spawn((
        Transform::from_translation(Vec3::new(7.0, 8.0, 10.0)),
        Stretch::default(),
    ));
    commands.spawn(HandleHolder {
        handle: asset_server.load("test_script.lua"),
    });
}

#[derive(Component)]
pub struct HandleHolder {
    handle: Handle<LuaScript>,
}
