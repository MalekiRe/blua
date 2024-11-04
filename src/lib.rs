pub mod asset_loader;

use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use bevy::asset::AssetLoader;
use bevy::prelude::*;
use piccolo::{Closure, Executor, Lua};
use send_wrapper::SendWrapper;
use crate::asset_loader::{LuaAssetCommunicator, LuaAssetLoader, LuaScript};

pub struct LuaPlugin;

impl Plugin for LuaPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset_loader::<LuaAssetLoader>()
            .init_asset::<LuaScript>();
        app.add_systems(Startup, insert_lua_vm);
        app.add_systems(Update, lua_asset_handling);
        app.add_systems(Update, run_every_tick);
    }
}

pub fn insert_lua_vm(world: &mut World) {
    world.insert_non_send_resource(LuaVm{ lua: Lua::full() });
}

pub fn lua_asset_handling(world: &mut World) {
    world.resource_scope(|world, lua_asset_communicator: Mut<LuaAssetCommunicator>| {
        let Some(mut lua_vm) = world.get_non_send_resource_mut::<LuaVm>() else { return };
        for (new_script_bytes, new_script_path) in lua_asset_communicator.lua_script_bytes_rx.try_iter() {
            let exec = lua_vm.lua.try_enter(|ctx| {
                let closure = Closure::load(ctx, Some(&*new_script_path.to_string()), Cursor::new(new_script_bytes))?;
                Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
            }).unwrap();
            let lua_script = LuaScript {
                exec: SendWrapper::new(exec),
            };
            lua_asset_communicator.lua_script_tx.send(lua_script).unwrap();
        }
    });
}

pub fn run_every_tick(world: &mut World) {
    let mut lua = world.remove_non_send_resource::<LuaVm>().unwrap();

    let mut lua_scripts = world.get_resource::<Assets<LuaScript>>().unwrap();

    for (_asset, script) in lua_scripts.iter() {
        lua.try_enter(|ctx| {
            ctx.fetch(script.exec.deref()).restart(ctx, function, ());
            Ok(())
        }).unwrap();
        lua.execute::<()>(script.exec.deref()).unwrap();
    }

    world.insert_non_send_resource(lua);
}

#[derive(Deref, DerefMut)]
pub struct LuaVm {
    lua: Lua,
}