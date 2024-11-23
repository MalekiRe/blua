pub mod asset_loader;
mod bevy_wrapper;
mod reflect_stuff;
pub mod userdata_stuff;

use crate::asset_loader::{LuaAssetCommunicator, LuaAssetLoader, LuaScript};
use crate::reflect_stuff::{
    ObjectFunctionRegistry, PtrState, ReflectPlugin, ReflectPtr, ReflectType, SystemParameter,
    WorldMut,
};
use crate::userdata_stuff::{UserDataPtr, ValueExt};
use bevy::ecs::component::ComponentId;
use bevy::ecs::system::SystemBuffer;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ptr::OwningPtr;
use bevy::reflect::func::{
    ArgList, ArgValue, DynamicFunction, FunctionError, FunctionInfo, FunctionRegistry, IntoReturn,
    ReflectFn, Return, TypedFunction,
};
use bevy::reflect::{impl_reflect, ReflectFromPtr, Typed};
use piccolo::{
    Callback, CallbackReturn, Closure, Context, Executor, IntoValue, Lua, Table, TypeError,
    UserData, Value, Variadic,
};
use send_wrapper::SendWrapper;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::ops::DerefMut;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Mutex;

pub struct LuaPlugin;

#[derive(Reflect)]
pub struct TableReflectWrapper {
    #[reflect(ignore)]
    table: Option<SendWrapper<Table<'static>>>,
}

impl TableReflectWrapper {
    pub unsafe fn new(table: Table) -> TableReflectWrapper {
        Self {
            table: Some(SendWrapper::new(std::mem::transmute(table))),
        }
    }
    pub unsafe fn take(self) -> Table<'static> {
        self.table.unwrap().take()
    }
}

impl Plugin for LuaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ReflectPlugin);
        app.init_asset_loader::<LuaAssetLoader>()
            .init_asset::<LuaScript>();
        app.add_systems(Startup, insert_lua_vm);
        app.add_systems(Update, lua_asset_handling);
        app.add_systems(Update, run_every_tick);
        app.register_object_function::<CommandQueueWrapper>(
            spawn.into_function().with_name("spawn"),
        );
    }
}

fn spawn_call(mut args: ArgList) -> Result<Return, FunctionError> {
    if args.len() != 2 {
        return Err(FunctionError::ArgCountMismatch {
            expected: 2,
            received: args.len(),
        });
    }

    let arg1 = args.take_mut::<CommandQueueWrapper>()?;
    let arg2 = args.take::<TableReflectWrapper>()?;
    spawn(arg1, arg2);
    Ok(Return::unit())
}

#[derive(Component)]
pub struct BluaScript(pub Handle<LuaScript>);

pub trait AppExtensionFunctionRegisterTrait {
    fn register_object_function<T: Reflect>(&mut self, function: DynamicFunction<'static>);
    fn register_non_self_object_function<T: Reflect + Typed>(
        &mut self,
        function: DynamicFunction<'static>,
    );
}

pub fn lua_wrapped_dynamic_function_call<'gc>(
    ctx: Context<'gc>,
    function: DynamicFunction<'static>,
    object_function_registry: Rc<RefCell<ObjectFunctionRegistry>>,
) -> Value<'gc> {
    Callback::from_fn(&ctx, move |context, _fuel, mut stack| {
        let mut args_list = ArgList::new();
        use bevy::prelude::Function;
        let args_uwu: Variadic<Vec<Value>> = stack.consume(context)?;
        for v in args_uwu {
            match v {
                Value::Nil => {
                    todo!()
                }
                Value::Boolean(bool) => {
                    args_list = args_list.push_owned(bool);
                }
                Value::Integer(int) => {
                    args_list = args_list.push_owned(int);
                }
                Value::Number(float) => {
                    args_list = args_list.push_owned(float);
                }
                Value::String(_) => {
                    todo!()
                }
                Value::Table(table) => {
                    args_list = args_list.push_owned(unsafe { TableReflectWrapper::new(table) });
                }
                Value::Function(_) => {
                    todo!()
                }
                Value::UserData(user_data) => {
                    if let Ok(reflect) = user_data.downcast_static::<ReflectPtr>() {
                        args_list = args_list
                            .push_mut(reflect.get_field_value_mut().as_partial_reflect_mut());
                    } else {
                        todo!()
                    }
                }
                Value::Thread(_) => {}
            }
        }
        let ret = function.call(args_list).unwrap();
        match ret {
            Return::Owned(mut owned) => {
                if let Some(awa) = owned.try_as_reflect().unwrap().downcast_ref::<f32>() {
                    stack.push_front(Value::Number(*awa as f64))
                }
                if let Some(awa) = owned.try_as_reflect().unwrap().downcast_ref::<f64>() {
                    stack.push_front(Value::Number(*awa))
                }

                if let Some(awa) = owned.try_as_reflect().unwrap().downcast_ref::<i32>() {
                    stack.push_front(Value::Integer(*awa as i64))
                }
                if let Some(awa) = owned.try_as_reflect().unwrap().downcast_ref::<i64>() {
                    stack.push_front(Value::Integer(*awa))
                }
                let reflect_ptr = ReflectPtr::new_boxed(
                    owned.try_into_reflect().unwrap(),
                    Rc::new(RefCell::new(PtrState::Valid)),
                    object_function_registry.clone(),
                );
                stack.push_front(reflect_ptr.into_value(&context));
            }
            Return::Ref(_) => {
                println!("todo return &");
            }
            Return::Mut(_) => {
                println!("todo return &mut");
            }
        }
        Ok(CallbackReturn::Return)
    })
    .into_value(ctx)
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
    fn register_non_self_object_function<T: Reflect + Typed>(
        &mut self,
        function: DynamicFunction<'static>,
    ) {
        self.init_non_send_resource::<Rc<RefCell<ObjectFunctionRegistry>>>();
        let mut object_function_registry = self
            .world_mut()
            .get_non_send_resource::<Rc<RefCell<ObjectFunctionRegistry>>>()
            .unwrap()
            .clone();
        let mut ofr1 = object_function_registry.clone();
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
        function_registry.register(function.clone()).unwrap();

        let world = self.world_mut();

        let type_id = TypeId::of::<T>();
        let things = T::type_info()
            .type_path()
            .split("::")
            .collect::<Vec<&str>>();

        // uncomment this if you wanna see the path of all the things aviable to you
        //println!("{:?}", things);
        world.init_non_send_resource::<LuaVm>();
        let mut lua = world.get_non_send_resource_mut::<LuaVm>().unwrap();
        lua.lua
            .try_enter(move |ctx| {
                let mut lua_table = ctx.globals();
                let len = things.len();
                let f = function.clone();
                for (i, item) in things.into_iter().enumerate() {
                    if i + 1 == len {
                        let name = function.name().unwrap().to_string();

                        let f = f.clone();
                        let function = lua_wrapped_dynamic_function_call(ctx, f, ofr1);

                        let t = match lua_table.get(ctx, item) {
                            Value::Nil => {
                                lua_table.set(ctx, item, Table::new(&ctx)).unwrap();
                                match lua_table.get(ctx, item) {
                                    Value::Table(table) => table,
                                    _ => unreachable!(),
                                }
                            }
                            Value::Table(table) => table,
                            _ => panic!("awa"),
                        };
                        t.set(ctx, name, function).unwrap();
                        break;
                    }
                    lua_table = match lua_table.get(ctx, item) {
                        Value::Nil => {
                            lua_table.set(ctx, item, Table::new(&ctx)).unwrap();
                            match lua_table.get(ctx, item) {
                                Value::Table(table) => table,
                                _ => unreachable!(),
                            }
                        }
                        Value::Table(table) => table,
                        _ => panic!("awa"),
                    };
                }
                Ok(())
            })
            .unwrap();
    }
}

#[derive(Reflect)]
struct HashMapWrapper {
    #[reflect(ignore)]
    hashmap: Option<SendWrapper<Vec<ReflectPtr>>>,
}

pub fn insert_lua_vm(world: &mut World) {
    world.init_non_send_resource::<LuaVm>();
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
        let mut command_queue = CommandQueueWrapper {
            commands: Default::default(),
        };
        for awa in awa.systems.iter_mut() {
            let stashed_function = &awa.lua_func;
            let mut ptr_states = vec![];
            let ofr1 = object_function_registry.clone();
            let (exec) = lua
                .try_enter(|ctx| {
                    let func = ctx.fetch(stashed_function);
                    let mut things = vec![];

                    for system_parameter in &mut awa.system_parameters {
                        let ptr_state = Rc::new(RefCell::new(PtrState::Valid));
                        let ptr_state2 = ptr_state.clone();
                        match system_parameter {
                            SystemParameter::Query((query, component_infos)) => {
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
                                            let value = unsafe {
                                                reflect_from_ptr.as_reflect_mut(x.as_mut())
                                            };
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
                                        let iterator_state =
                                            ctx.fetch(&iterator_state).into_value(ctx);
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
                                things.push(t.into_value(ctx));
                            }
                            SystemParameter::CommandQueue => {
                                let reflect_mut = ReflectPtr::new(
                                    &mut command_queue,
                                    ptr_state2.clone(),
                                    ofr1.clone(),
                                );
                                things.push(reflect_mut.into_value(&ctx));
                                ptr_states.push(ptr_state);
                            }
                        }
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
            command_queue.commands.apply(world);
        }
    }

    world.insert_resource(lua_scripts);
    world.insert_non_send_resource(lua);
}

#[derive(Reflect, Deref, DerefMut)]
pub struct CommandQueueWrapper {
    #[reflect(ignore)]
    pub commands: CommandQueue,
}
pub fn spawn(this: &mut CommandQueueWrapper, table: TableReflectWrapper) {
    this.push(move |world: &mut World| {
        let table = table.table.unwrap().take();
        for (_key, value) in table {
            let Ok(value) = value.as_static_user_data::<ReflectPtr>() else {
                println!("passed non reflect to spawn function");
                continue;
            };
            let type_id = unsafe { &mut *value.get_data() }
                .get_represented_type_info()
                .unwrap()
                .type_id();
            match &value.data {
                ReflectType::Ptr(_) => {}
                ReflectType::Boxed(boxed) => {
                    let thing_to_add = boxed.borrow_mut().take().unwrap();
                    let component_id: ComponentId = world.components().get_id(type_id).unwrap();
                    let mut e: EntityWorldMut = world.spawn_empty();
                    let data_ptr = Box::into_raw(thing_to_add) as *mut u8;
                    unsafe {
                        e.insert_by_id(
                            component_id,
                            OwningPtr::new(NonNull::new(data_ptr).unwrap()),
                        )
                    };
                }
            }
        }
    });
}

#[derive(Deref, DerefMut)]
pub struct LuaVm {
    lua: Lua,
}
impl Default for LuaVm {
    fn default() -> Self {
        Self { lua: Lua::full() }
    }
}
