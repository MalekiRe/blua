use bevy::app::App;
use bevy::asset::{AssetServer, Handle};
use bevy::prelude::ReflectComponent;
use bevy::prelude::*;
use bevy::prelude::{Commands, Component, Reflect, Res, Startup, Transform, Vec3};
use bevy::DefaultPlugins;
use blua::asset_loader::LuaScript;
use blua::{AppExtensionFunctionRegisterTrait, LuaPlugin};
fn main() {
    let mut app = App::default();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .add_plugins(LuaPlugin);
    app.add_systems(Startup, setup);
    app.register_type::<Stretch>();
    app.world_mut().register_component::<Stretch>();
    app.register_object_function::<Stretch>(Stretch::get_sum.into_function().with_name("get_sum"));
    app.register_object_function::<Stretch>(
        Stretch::get_sum_with
            .into_function()
            .with_name("get_sum_with"),
    );
    app.register_object_function::<Time<()>>(
        Time::<()>::elapsed_secs
            .into_function()
            .with_name("elapsed_secs"),
    );
    app.run();
}

#[derive(Component, Reflect, PartialEq, Debug, Default)]
#[reflect(Component, Default, PartialEq, Debug)]
pub struct Stretch {
    pub x: f32,
    pub y: f32,
}

impl Stretch {
    pub fn get_sum(&self) -> f32 {
        self.x + self.y
    }
    pub fn get_sum_with(&self, other: f64) -> f64 {
        (self.x + self.y + other as f32) as f64
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Transform::from_translation(Vec3::new(3.0, 4.0, 6.0)),
        Stretch { x: 0.5, y: 0.75 },
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
