pub mod asset_loader;
mod reflect_stuff;
pub mod userdata_stuff;

use crate::asset_loader::{LuaAssetCommunicator, LuaAssetLoader, LuaScript};
use crate::reflect_stuff::{LuaSystems, ReflectPlugin, ReflectPtr, WorldMut};
use crate::userdata_stuff::UserDataPtr;
use bevy::prelude::*;
use bevy::reflect::ReflectFromPtr;
use piccolo::{
    Callback, CallbackReturn, Closure, Context, Executor, IntoValue, Lua, Table, UserData, Variadic,
};
use std::io::Cursor;
use std::ops::DerefMut;
use std::sync::Mutex;

pub struct LuaPlugin;

impl Plugin for LuaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ReflectPlugin);
        app.init_asset_loader::<LuaAssetLoader>()
            .init_asset::<LuaScript>();
        app.add_systems(Startup, insert_lua_vm);
        app.add_systems(Update, lua_asset_handling);
        app.add_systems(Update, run_every_tick);
    }
}

pub fn insert_lua_vm(world: &mut World) {
    world.insert_non_send_resource(LuaVm { lua: Lua::full() });
}

pub fn lua_asset_handling(world: &mut World) {
    world.resource_scope(|world, lua_asset_communicator: Mut<LuaAssetCommunicator>| {
        let Some(mut lua) = world.remove_non_send_resource::<LuaVm>() else {
            return;
        };

        let mut lua_app = WorldMut::new(world);
        for (new_script_bytes, new_script_path) in
            lua_asset_communicator.lua_script_bytes_rx.try_iter()
        {
            let exec = lua
                .try_enter(|ctx| {
                    let lua_app_value = lua_app.clone().into_value(&ctx);
                    let closure = Closure::load(
                        ctx,
                        Some(&*new_script_path.to_string()),
                        Cursor::new(new_script_bytes),
                    )?;
                    Ok(ctx.stash(Executor::start(ctx, closure.into(), lua_app_value)))
                })
                .unwrap();
            lua.execute::<()>(&exec).unwrap();
        }
        lua_app.this.take().unwrap();
        drop(lua_app);
        world.insert_non_send_resource(lua);
    });
}

pub struct IteratorState {
    pub components: Vec<Vec<ReflectPtr>>,
}

impl IteratorState {
    fn iterator_fn<'gc>(ctx: &Context<'gc>) -> Callback<'gc> {
        Callback::from_fn(&ctx, |ctx, _fuel, mut stack| {
            let state: UserData = stack.consume(ctx)?;

            let state = state.downcast_static::<Mutex<IteratorState>>()?;

            let mut state = state.lock().unwrap();
            let state = state.deref_mut();
            if state.components.is_empty() {
                return Ok(CallbackReturn::Return);
            }
            let next_value_vec = state.components.remove(0);

            for value in next_value_vec {
                stack.push_back(value.into_value(&ctx));
            }

            Ok(CallbackReturn::Return)
        })
    }
}

pub fn run_every_tick(world: &mut World) {
    let mut lua = world.remove_non_send_resource::<LuaVm>().unwrap();

    let mut lua_systems = world.remove_non_send_resource::<LuaSystems>().unwrap();
    let app_registry = world.get_resource::<AppTypeRegistry>().unwrap().clone();
    for awa in lua_systems.iter_mut() {
        let stashed_function = &awa.lua_func;
        let exec = lua
            .try_enter(|ctx| {
                let func = ctx.fetch(stashed_function);
                let mut things = vec![];
                for (query, component_infos) in &mut awa.queries {
                    let items = query.iter_mut(world).collect::<Vec<_>>();
                    let items = items
                        .into_iter()
                        .map(|mut a| {
                            let mut values = vec![];
                            for (component_id, type_id) in component_infos.iter() {
                                let mut x = a.get_mut_by_id(*component_id).unwrap();
                                let app_registry = app_registry.read();
                                let reflect_data = app_registry.get(*type_id).unwrap();
                                let reflect_from_ptr =
                                    reflect_data.data::<ReflectFromPtr>().unwrap();
                                let value = unsafe { reflect_from_ptr.as_reflect_mut(x.as_mut()) };
                                values.push(ReflectPtr::new(value));
                            }
                            values
                        })
                        .collect::<Vec<_>>();
                    let iterator_state = ctx.stash(UserData::new_static(
                        &ctx,
                        Mutex::new(IteratorState {
                            components: items.clone(),
                        }),
                    ));
                    let t = Table::new(&ctx);
                    t.set(
                        ctx,
                        "iter",
                        Callback::from_fn(&ctx, move |ctx, _fuel, mut stack| {
                            let iterator_state = ctx.fetch(&iterator_state).into_value(ctx);
                            stack.replace(ctx, (IteratorState::iterator_fn(&ctx), iterator_state));

                            Ok(CallbackReturn::Return)
                        }),
                    )
                    .unwrap();
                    things.push(t);
                }
                Ok(ctx.stash(Executor::start(ctx, func, Variadic(things))))
            })
            .unwrap();
        lua.execute::<()>(&exec).unwrap();
    }

    world.insert_non_send_resource(lua_systems);
    world.insert_non_send_resource(lua);
}

#[derive(Deref, DerefMut)]
pub struct LuaVm {
    lua: Lua,
}
