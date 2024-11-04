use bevy::app::App;
use bevy::asset::{AssetServer, Handle};
use bevy::DefaultPlugins;
use bevy::prelude::{Commands, Component, Res, Startup};
use blua::asset_loader::LuaScript;
use blua::LuaPlugin;

fn main() {
    let mut app = App::default();
    app.add_plugins(DefaultPlugins).add_plugins(LuaPlugin);
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(
        HandleHolder { handle: asset_server.load("test_script.lua") }
    );
}

#[derive(Component)]
pub struct HandleHolder {
    handle: Handle<LuaScript>,
}