pub mod asset_loader;
mod bevy_wrapper;
mod reflect_stuff;
pub mod userdata_stuff;

use crate::asset_loader::{LuaAssetCommunicator, LuaAssetLoader, LuaScript};
use crate::reflect_stuff::{ObjectFunctionRegistry, PtrState, ReflectPlugin, ReflectPtr, WorldMut};
use crate::userdata_stuff::{UserDataPtr, ValueExt};
use bevy::prelude::*;
use bevy::reflect::func::{DynamicFunction, FunctionRegistry};
use bevy::reflect::ReflectFromPtr;
use piccolo::{
    Callback, CallbackReturn, Closure, Context, Executor, IntoValue, Lua, Table, UserData, Value,
    Variadic,
};
use send_wrapper::SendWrapper;
use std::any::TypeId;
use std::cell::RefCell;
use std::io::Cursor;
use std::ops::DerefMut;
use std::rc::Rc;
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

#[derive(Component)]
pub struct BluaScript(pub Handle<LuaScript>);

pub trait AppExtensionFunctionRegisterTrait {
    fn register_object_function<T: Reflect>(&mut self, function: DynamicFunction<'static>);
}
impl AppExtensionFunctionRegisterTrait for App {
    fn register_object_function<T: Reflect>(&mut self, function: DynamicFunction<'static>) {
        self.init_non_send_resource::<Rc<RefCell<ObjectFunctionRegistry>>>();
        let mut object_function_registry = self
            .world_mut()
            .get_non_send_resource::<Rc<RefCell<ObjectFunctionRegistry>>>()
            .unwrap();
        if !object_function_registry
            .borrow()
            .contains_key(&TypeId::of::<T>())
        {
            object_function_registry
                .borrow_mut()
                .insert(TypeId::of::<T>(), FunctionRegistry::default());
        }
        let mut awa = object_function_registry.borrow_mut();
        let function_registry = awa.get_mut(&TypeId::of::<T>()).unwrap();
        function_registry.register(function).unwrap();
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
            let mut systems_vec = Rc::new(RefCell::new(Some(Vec::new())));
            let exec = lua
                .try_enter(|ctx| {
                    let user_data = UserData::new_static(&ctx, systems_vec.clone());
                    ctx.set_global("__systems_vec", user_data).unwrap();
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
            lua_asset_communicator
                .lua_script_tx
                .send(LuaScript {
                    systems: SendWrapper::new(systems_vec.take().unwrap()),
                })
                .unwrap();
        }
        lua.try_enter(|ctx| {
            ctx.set_global("__systems_vec", Value::Nil).unwrap();
            Ok(CallbackReturn::Return)
        })
        .unwrap();
        lua_app.this.take().unwrap();
        drop(lua_app);
        world.insert_non_send_resource(lua);
    });
}

pub struct IteratorState {
    pub components: Vec<Vec<ReflectPtr>>,
    pub ptr_state: Rc<RefCell<PtrState>>,
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

    let mut lua_scripts = world.remove_resource::<Assets<LuaScript>>().unwrap();

    let app_registry = world.get_resource::<AppTypeRegistry>().unwrap().clone();
    world.init_non_send_resource::<Rc<RefCell<ObjectFunctionRegistry>>>();
    let object_function_registry = world
        .get_non_send_resource::<Rc<RefCell<ObjectFunctionRegistry>>>()
        .unwrap()
        .clone();
    for (_, awa) in lua_scripts.iter_mut() {
        for awa in awa.systems.iter_mut() {
            let stashed_function = &awa.lua_func;
            let mut ptr_states = vec![];
            let ofr1 = object_function_registry.clone();
            let (exec) = lua
                .try_enter(|ctx| {
                    let func = ctx.fetch(stashed_function);
                    let mut things = vec![];
                    for (query, component_infos) in &mut awa.queries {
                        let ptr_state = Rc::new(RefCell::new(PtrState::Valid));
                        let ptr_state2 = ptr_state.clone();
                        let items = query.iter_mut(world).collect::<Vec<_>>();
                        let items = items
                            .into_iter()
                            .map(|mut a| {
                                let mut values = vec![];
                                //a.components();
                                for (component_id, type_id) in component_infos.iter() {
                                    let mut x = a.get_mut_by_id(*component_id).unwrap();
                                    let app_registry = app_registry.read();
                                    let reflect_data = app_registry.get(*type_id).unwrap();
                                    let reflect_from_ptr =
                                        reflect_data.data::<ReflectFromPtr>().unwrap();
                                    let value =
                                        unsafe { reflect_from_ptr.as_reflect_mut(x.as_mut()) };
                                    values.push(ReflectPtr::new(
                                        value,
                                        ptr_state2.clone(),
                                        ofr1.clone(),
                                    ));
                                }
                                values
                            })
                            .collect::<Vec<_>>();
                        let iterator_state = ctx.stash(UserData::new_static(
                            &ctx,
                            Mutex::new(IteratorState {
                                components: items,
                                ptr_state: ptr_state.clone(),
                            }),
                        ));
                        ptr_states.push(ptr_state);
                        let t = Table::new(&ctx);
                        t.set(
                            ctx,
                            "iter",
                            Callback::from_fn(&ctx, move |ctx, _fuel, mut stack| {
                                let iterator_state = ctx.fetch(&iterator_state).into_value(ctx);
                                *iterator_state
                                    .as_static_user_data::<Mutex<IteratorState>>()
                                    .unwrap()
                                    .lock()
                                    .unwrap()
                                    .ptr_state
                                    .borrow_mut() = PtrState::Valid;
                                stack.replace(
                                    ctx,
                                    (IteratorState::iterator_fn(&ctx), iterator_state),
                                );

                                Ok(CallbackReturn::Return)
                            }),
                        )
                        .unwrap();
                        things.push(t);
                    }
                    Ok(ctx.stash(Executor::start(ctx, func, Variadic(things))))
                })
                .unwrap();
            if let Err(err) = lua.execute::<()>(&exec) {
                println!("error running lua script didn't work: {err}");
            }
            for ptr_state in ptr_states.iter() {
                *ptr_state.borrow_mut() = PtrState::Invalid;
            }
        }
    }

    world.insert_resource(lua_scripts);
    world.insert_non_send_resource(lua);
}

#[derive(Deref, DerefMut)]
pub struct LuaVm {
    lua: Lua,
}
