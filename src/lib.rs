pub mod asset_loader;
pub mod userdata_stuff;

use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use bevy::asset::AssetLoader;
use bevy::prelude::*;
use piccolo::{Callback, CallbackReturn, Closure, Context, Executor, Function, IntoValue, Lua, StashedCallback, StashedFunction, Table, UserData, Value};
use send_wrapper::SendWrapper;
use crate::asset_loader::{LuaAssetCommunicator, LuaAssetLoader, LuaScript};
use crate::userdata_stuff::UserDataWrapper;

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
    world.insert_non_send_resource(AppInfo::default());
}

/*
local app = ...
local system = app:system():resource("some_resource", SomeResource):query("my_query", Query:entity():component(ComponentA):component(Component(B)):with(ComponentC):without(ComponentD)
app:register_system(Update, my_system, system);


function my_system(some_resource, my_query)
  for entity, componenta, componentb in my_query:iter()
    print(entity)
  end
  local some_val = some_resource:get_something()
end
 */

#[derive(Default)]
pub struct AppInfo {
    systems: Vec<SystemInfo>,
}

#[derive(Clone)]
pub struct SystemInfo {
    schedule: String,
    system: LuaSystem,
    func: StashedFunction,
}

//TODO
#[derive(Clone)]
struct LuaSystem;

type LuaApp = UserDataWrapper<World, Arc<Mutex<Option<AppInfo>>>>;

impl UserDataWrapper<World, Arc<Mutex<Option<AppInfo>>>> {
    pub fn metatable<'gc>(ctx: &Context<'gc>) -> Table<'gc> {
        let mut metatable = Table::new(&ctx);


        metatable.set(*ctx,
        "__index", Callback::from_fn(&ctx, move |ctx, _fuel, mut stack| {
                let (_this, key): (&LuaApp, Value) = stack.consume(ctx)?;
                let key = key.to_string();
                match key.as_str() {
                    "register_system" => {
                        println!("pushing register system to front");
                        stack.push_front(Self::register_system(ctx).into_value(ctx));
                    }
                    &_ => {}
                }
                Ok(CallbackReturn::Return)
            })).unwrap();

        metatable
    }
    fn register_system<'gc>(ctx: Context<'gc>) -> Callback<'gc> {
        Callback::from_fn(&ctx, move |ctx, _fuel, mut stack| {
            println!("in register system");
            let (mut this, lua_system): (&LuaApp, Function<'gc>) = stack.consume(ctx)?;
            let stashed_function = ctx.stash(lua_system);
            this.other.lock().unwrap().as_mut().unwrap().systems.push(SystemInfo {
                schedule: "".to_string(),
                system: LuaSystem,
                func: stashed_function,
            });
            println!("awa");
            Ok(CallbackReturn::Return)
        })
    }
}

pub fn lua_asset_handling(world: &mut World) {
    world.resource_scope(|world, lua_asset_communicator: Mut<LuaAssetCommunicator>| {
        let Some(mut lua) = world.remove_non_send_resource::<LuaVm>() else { return };
        let Some(app_info) = world.remove_non_send_resource::<AppInfo>() else { return };
        let lua_app = LuaApp::new(world, Arc::new(Mutex::new(Some(app_info))));
        for (new_script_bytes, new_script_path) in lua_asset_communicator.lua_script_bytes_rx.try_iter() {
            let exec = lua.try_enter(|ctx| {
                let lua_app_value = lua_app.clone().into_value(&ctx, LuaApp::metatable(&ctx));
                let closure = Closure::load(ctx, Some(&*new_script_path.to_string()), Cursor::new(new_script_bytes))?;
                Ok(ctx.stash(Executor::start(ctx, closure.into(), lua_app_value)))
            }).unwrap();
            lua.execute::<()>(&exec).unwrap();
            let lua_script = LuaScript {
                /*exec: SendWrapper::new(exec),*/
            };
            lua_asset_communicator.lua_script_tx.send(lua_script).unwrap();
        }
        lua_app.data.take().unwrap();
        let app_info = lua_app.other.lock().unwrap().take().unwrap();
        drop(lua_app);
        world.insert_non_send_resource(app_info);
        world.insert_non_send_resource(lua);
    });
}

pub fn run_every_tick(world: &mut World) {
    let mut lua = world.remove_non_send_resource::<LuaVm>().unwrap();

    let mut lua_scripts = world.get_resource::<Assets<LuaScript>>().unwrap();

    let app_info = world.remove_non_send_resource::<AppInfo>().unwrap();

    for awa in app_info.systems.iter() {
        let stashed_function = awa.func.clone();
        let exec = lua.try_enter(|ctx| {

            let func = ctx.fetch(&stashed_function);
            Ok(ctx.stash(Executor::start(ctx, func, ())))
        }).unwrap();
        lua.execute::<()>(&exec).unwrap();
    }

    /*for (_asset, script) in lua_scripts.iter() {
        let executor = lua.try_enter(|ctx| {
            ctx.stash(Executor::start(ctx, , lua_app_value));
            ctx.fetch(script.exec.deref())
           /* ctx.fetch(script.exec.deref()).restart(ctx, , ());*/
            Ok(())
        }).unwrap();
        lua.execute::<()>(script.exec.deref()).unwrap();
    }*/

    world.insert_non_send_resource(lua);
    world.insert_non_send_resource(app_info);
}

#[derive(Deref, DerefMut)]
pub struct LuaVm {
    lua: Lua,
}