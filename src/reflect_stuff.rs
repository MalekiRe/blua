use crate::userdata_stuff::{UserDataPtr, ValueExt};
use crate::{lua_wrapped_dynamic_function_call, HashMapWrapper, LuaVm, TableReflectWrapper};
use bevy::ecs::component::{ComponentDescriptor, ComponentId};
use bevy::ecs::prelude::AppFunctionRegistry;
use bevy::ecs::world::{CommandQueue, FilteredEntityMut};
use bevy::prelude::*;
use bevy::reflect::func::{ArgList, FunctionRegistry, Return};
use piccolo::{
    Callback, CallbackReturn, Context, FromValue, Function, IntoValue, StashedFunction, Table,
    TypeError, UserData, Value, Variadic,
};
use send_wrapper::SendWrapper;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Copy, Clone, Debug)]
pub enum ComponentType {
    Ref((ComponentId, TypeId)),
    Mut((ComponentId, TypeId)),
}

#[derive(Copy, Clone, Debug)]
pub struct CommandQueueMarker;

pub struct LuaSystem {
    pub lua_func: StashedFunction,
    pub system_parameters: Vec<SystemParameter>,
}

pub enum SystemParameter {
    Query((QueryState<FilteredEntityMut<'static>>, Vec<ComponentType>)),
    CommandQueue,
}

pub struct ReflectPtr {
    pub data: ReflectType,
    path: String,
    ptr_state: Rc<RefCell<PtrState>>,
    function_registry: Rc<RefCell<ObjectFunctionRegistry>>,
}

pub enum ReflectType {
    PtrMut(*mut dyn Reflect),
    PtrRef(*const dyn Reflect),
    Boxed(Rc<RefCell<Option<Box<dyn Reflect>>>>),
}

impl Clone for ReflectType {
    fn clone(&self) -> Self {
        match self {
            ReflectType::Boxed(boxed) => ReflectType::Boxed(boxed.clone()),
            ReflectType::PtrMut(ptr) => ReflectType::PtrMut(*ptr),
            ReflectType::PtrRef(ptr) => ReflectType::PtrRef(*ptr),
        }
    }
}

#[derive(Default, Deref, DerefMut)]
pub struct ObjectFunctionRegistry {
    map: HashMap<TypeId, FunctionRegistry>,
}

#[derive(PartialEq, Debug)]
pub enum PtrState {
    Valid,
    Invalid,
}

impl ReflectPtr {
    pub fn new_mut(
        reflect: &mut dyn Reflect,
        ptr_state: Rc<RefCell<PtrState>>,
        function_registry: Rc<RefCell<ObjectFunctionRegistry>>,
    ) -> Self {
        Self {
            data: ReflectType::PtrMut(reflect as *mut dyn Reflect),
            path: "".to_string(),
            ptr_state,
            function_registry,
        }
    }
    pub fn new_ref(
        reflect: &dyn Reflect,
        ptr_state: Rc<RefCell<PtrState>>,
        function_registry: Rc<RefCell<ObjectFunctionRegistry>>,
    ) -> Self {
        Self {
            data: ReflectType::PtrRef(reflect as *const dyn Reflect),
            path: "".to_string(),
            ptr_state,
            function_registry,
        }
    }
    pub fn new_boxed(
        reflect: Box<dyn Reflect>,
        ptr_state: Rc<RefCell<PtrState>>,
        function_registry: Rc<RefCell<ObjectFunctionRegistry>>,
    ) -> Self {
        Self {
            data: ReflectType::Boxed(Rc::new(RefCell::new(Some(reflect)))),
            path: "".to_string(),
            ptr_state,
            function_registry,
        }
    }
    pub fn get_field_value_ref(&self) -> &dyn Reflect {
        if &*self.ptr_state.borrow() == &PtrState::Invalid {
            panic!("invalid pointer state, saved outside of valid area")
        }
        let mut reflect = unsafe { &*self.get_data() };
        for field in self.path.split(".") {
            if field.is_empty() {
                continue;
            }
            reflect = reflect
                .as_reflect()
                .reflect_ref()
                .as_struct()
                .unwrap()
                .field(field)
                .unwrap()
                .try_as_reflect()
                .unwrap();
        }
        reflect
    }
    pub fn get_field_value_mut(&self) -> &mut dyn Reflect {
        if &*self.ptr_state.borrow() == &PtrState::Invalid {
            panic!("invalid pointer state, saved outside of valid area")
        }
        let mut reflect = unsafe { &mut *self.get_data_mut().unwrap() };
        for field in self.path.split(".") {
            if field.is_empty() {
                continue;
            }
            reflect = reflect
                .as_reflect_mut()
                .reflect_mut()
                .as_struct()
                .unwrap()
                .field_mut(field)
                .unwrap()
                .try_as_reflect_mut()
                .unwrap();
        }
        reflect
    }
}

impl UserDataPtr for ReflectPtr {
    type Data = dyn Reflect;

    fn get_data_mut(&self) -> Option<*mut Self::Data> {
        match &self.data {
            ReflectType::PtrMut(ptr) => Some(*ptr),
            ReflectType::PtrRef(_) => None,
            ReflectType::Boxed(boxed) => {
                Some(((boxed.borrow_mut().as_mut().unwrap()).as_mut() as *mut dyn Reflect).clone())
            }
        }
    }

    fn get_data(&self) -> *const Self::Data {
        match &self.data {
            ReflectType::PtrMut(ptr) => *ptr,
            ReflectType::Boxed(boxed) => {
                ((boxed.borrow().as_ref().unwrap()).as_ref() as *const dyn Reflect).clone()
            }
            ReflectType::PtrRef(ptr) => *ptr,
        }
    }

    fn edit_metatable<'gc>(&self, ctx: &Context<'gc>, metatable: &mut Table<'gc>) {
        metatable
            .set(
                *ctx,
                "__add",
                Callback::from_fn(ctx, move |ctx, _fuel, mut stack| {
                    let (this, rhs): (&Self, Value) = stack.consume(ctx)?;
                    match rhs {
                        Value::Integer(integer) => {
                            if let Some(awa) = this.get_field_value_ref().downcast_ref::<i32>() {
                                stack.push_front(Value::Integer(*awa as i64 + integer))
                            }
                        }
                        Value::Number(number) => {
                            if let Some(awa) = this.get_field_value_ref().downcast_ref::<f32>() {
                                stack.push_front(Value::Number(*awa as f64 + number))
                            }
                        }
                        Value::UserData(_) => {
                            todo!()
                        }
                        _ => {
                            stack.push_front(Value::Nil);
                            println!("tried to add something that can't be added")
                        }
                    }
                    Ok(CallbackReturn::Return)
                }),
            )
            .unwrap();
    }

    fn lua_to_string(&self) -> String {
        format!("{:?}", self.get_field_value_ref())
    }

    // TODO safe mutability by seperating mut vs ref pointers
    fn lua_index<'gc>(&self, ctx: &Context<'gc>, key: &str) -> Value<'gc> {
        let mut reflect_ptr = self.clone();
        if let Some(function_registry) = self
            .function_registry
            .borrow()
            .get(&self.get_field_value_ref().reflect_type_info().type_id())
        {
            if let Some(function) = function_registry.get(key) {
                return lua_wrapped_dynamic_function_call(
                    *ctx,
                    function.clone(),
                    self.function_registry.clone(),
                );
            }
        }
        // this is the case where it's not in the function registry
        reflect_ptr.path.push('.');
        reflect_ptr.path.push_str(key);
        reflect_ptr.into_value(ctx)
    }

    fn lua_new_index<'gc>(&self, _ctx: &Context<'gc>, key: &str, new_value: Value<'gc>) {
        let mut reflect_ptr = self.clone();
        reflect_ptr.path.push('.');
        reflect_ptr.path.push_str(key);
        match new_value {
            Value::Number(n) => {
                let mut reflect_field: &mut dyn Reflect = reflect_ptr.get_field_value_mut();
                reflect_field.set(Box::new(n as f32)).unwrap();
            }
            Value::Integer(i) => {
                let mut reflect_field: &mut dyn Reflect = reflect_ptr.get_field_value_mut();
                if let Err(awa) = reflect_field.set(Box::new(i as i32)) {
                    println!("error setting value: {:?}", awa);
                }
            }
            Value::Boolean(b) => {
                let mut reflect_field: &mut dyn Reflect = reflect_ptr.get_field_value_mut();
                reflect_field.set(Box::new(b)).unwrap();
            }
            Value::String(_s) => {
                todo!()
            }
            Value::UserData(data) => {
                if reflect_ptr.path.is_empty() {
                    reflect_ptr.data = data.downcast_static::<ReflectPtr>().unwrap().data.clone();
                    reflect_ptr.path = String::default();
                    reflect_ptr.ptr_state = data
                        .downcast_static::<ReflectPtr>()
                        .unwrap()
                        .ptr_state
                        .clone();
                } else {
                    let reflect_field: &mut dyn Reflect = reflect_ptr.get_field_value_mut();
                    let reflect = data
                        .downcast_static::<ReflectPtr>()
                        .unwrap()
                        .get_field_value_mut()
                        .clone_value();
                    reflect_field.apply(&*reflect);
                    //reflect_field.set(reflect).unwrap();
                }
            }
            _ => {}
        }
    }
}

impl Clone for ReflectPtr {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            path: self.path.clone(),
            ptr_state: self.ptr_state.clone(),
            function_registry: self.function_registry.clone(),
        }
    }
}

impl<'gc> FromValue<'gc> for &'gc ReflectPtr {
    fn from_value(ctx: Context<'gc>, value: Value<'gc>) -> Result<Self, TypeError> {
        ReflectPtr::from_value_2(ctx, value)
    }
}

pub struct WorldMut {
    pub(crate) this: Option<*mut World>,
}

impl Clone for WorldMut {
    fn clone(&self) -> Self {
        WorldMut {
            this: Some(self.this.unwrap()),
        }
    }
}

impl WorldMut {
    pub fn new(world: &mut World) -> Self {
        Self {
            this: Some(world as *mut World),
        }
    }
}

impl<'gc> FromValue<'gc> for &'gc WorldMut {
    fn from_value(ctx: Context<'gc>, value: Value<'gc>) -> Result<Self, TypeError> {
        WorldMut::from_value_2(ctx, value)
    }
}

impl UserDataPtr for WorldMut {
    type Data = World;

    fn get_data_mut(&self) -> Option<*mut Self::Data> {
        Some(self.this.unwrap())
    }

    fn get_data(&self) -> *const Self::Data {
        self.this.unwrap() as *const Self::Data
    }

    fn edit_metatable<'gc>(&self, _ctx: &Context<'gc>, _table: &mut Table<'gc>) {}

    fn lua_to_string(&self) -> String {
        "app".to_string()
    }

    fn lua_index<'gc>(&self, ctx: &Context<'gc>, key: &str) -> Value<'gc> {
        match key {
            "query" => Self::query(ctx).into_value(*ctx),
            "register_system" => Self::register_system(ctx).into_value(*ctx),
            &_ => Value::Nil,
        }
    }

    fn lua_new_index<'gc>(&self, _ctx: &Context<'gc>, _key: &str, _new_value: Value<'gc>) {}
}

impl WorldMut {
    pub fn query<'gc>(ctx: &Context<'gc>) -> Callback<'gc> {
        Callback::from_fn(ctx, move |ctx, _fuel, mut stack| {
            let args: Value = stack.consume(ctx)?;

            stack.push_front(args.into_value(ctx));

            Ok(CallbackReturn::Return)
        })
    }
    pub fn register_system<'gc>(ctx: &Context<'gc>) -> Callback<'gc> {
        Callback::from_fn(ctx, move |ctx, _fuel, mut stack| {
            let systems_vec = ctx
                .globals()
                .get::<_, Value>(ctx, "__systems_vec")
                .unwrap()
                .as_static_user_data::<Rc<RefCell<Option<Vec<LuaSystem>>>>>()?;
            let systems_vec = systems_vec.clone();

            let (this, system, system_params): (&WorldMut, Value, Table) = stack.consume(ctx)?;

            let function: Function = Function::from_value(ctx, system)?;

            let world = unsafe { &mut *this.get_data_mut().unwrap() };

            let mut system_parameters = vec![];

            for (_, system_parameter) in system_params.into_iter() {
                //println!("hi");
                // TODO add resources and other things here too
                if system_parameter
                    .as_static_user_data::<CommandQueueMarker>()
                    .is_ok()
                {
                    system_parameters.push(SystemParameter::CommandQueue);
                    //println!("hello");
                    continue;
                }

                let table = Table::from_value(ctx, system_parameter)?;
                //println!("UwU");
                let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(world);
                //println!("UwU");
                //TODO we might want to restrict this to something like mut vs ref components
                let mut components = vec![];
                for (_, component_type) in table.into_iter() {
                    let component_type = UserData::from_value(ctx, component_type)?;
                    let component_type = component_type
                        .downcast_static::<ComponentType>()
                        .unwrap()
                        .clone();
                    match component_type {
                        ComponentType::Ref((component_id, type_id)) => {
                            query_builder.ref_id(component_id);
                            components.push(component_type);
                        }
                        ComponentType::Mut((component_id, type_id)) => {
                            query_builder.mut_id(component_id);
                            components.push(component_type);
                        }
                    }
                }
                let query_state = query_builder.build();
                system_parameters.push(SystemParameter::Query((query_state, components)));
            }
            let stashed_function = ctx.stash(function);
            systems_vec.borrow_mut().as_mut().unwrap().push(LuaSystem {
                lua_func: stashed_function,
                system_parameters,
            });
            Ok(CallbackReturn::Return)
        })
    }
}

pub struct ReflectPlugin;

impl Plugin for ReflectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, register_components_and_markers);
    }
}

fn register_components_and_markers(world: &mut World) {
    world.resource_scope(|world, registry: Mut<AppTypeRegistry>| {
        let mut lua = world.remove_non_send_resource::<LuaVm>().unwrap();
        lua.try_enter(|ctx| {
            ctx.globals()
                .set(
                    ctx,
                    "Commands",
                    UserData::new_static(&ctx, CommandQueueMarker).into_value(ctx),
                )
                .unwrap();
            Ok(CallbackReturn::Return)
        })
        .unwrap();
        for a in registry.read().iter() {
            //println!("{}", a.type_info().type_path());
        }
        let ids = registry
            .read()
            .iter()
            .map(|a| a.type_id())
            .collect::<Vec<_>>();
        let mut awa2: HashMap<TypeId, ComponentId> = HashMap::new();
        for id in ids {
            if let Some(awa) = registry.write().get_type_data::<ReflectComponent>(id) {
                let c = awa.register_component(world);
                awa2.insert(id, c);
            }
        }
        world.insert_non_send_resource(awa2);
        for item in registry.read().iter() {
            let component_id = match world.components().get_id(item.type_id()) {
                None => continue,
                Some(component_id) => component_id,
            };

            let type_id = item.type_id();
            let things = item
                .type_info()
                .type_path()
                .split("::")
                .collect::<Vec<&str>>();

            // uncomment this if you wanna see the path of all the things aviable to you
            //println!("{:?}", things);

            lua.try_enter(|ctx| {
                let mut lua_table = ctx.globals();
                let len = things.len();
                for (i, item) in things.into_iter().enumerate() {
                    if i + 1 == len {
                        let t = match lua_table.get(ctx, item).unwrap() {
                            Value::Nil => {
                                lua_table.set(ctx, item, Table::new(&ctx)).unwrap();
                                match lua_table.get(ctx, item).unwrap() {
                                    Value::Table(table) => table,
                                    _ => unreachable!(),
                                }
                            }
                            Value::Table(table) => table,
                            _ => panic!("awa"),
                        };

                        if item == "Transform" {
                            //println!("THIS ONE WAS TRANSOFMR");
                            //println!("type id of the transfomr is: {:?}", type_id);
                        }

                        t.set(
                            ctx,
                            "ref",
                            UserData::new_static(&ctx, ComponentType::Ref((component_id, type_id))),
                        )
                        .unwrap();
                        t.set(
                            ctx,
                            "mut",
                            UserData::new_static(&ctx, ComponentType::Mut((component_id, type_id))),
                        )
                        .unwrap();

                        break;
                    }
                    lua_table = match lua_table.get(ctx, item).unwrap() {
                        Value::Nil => {
                            lua_table.set(ctx, item, Table::new(&ctx)).unwrap();
                            match lua_table.get(ctx, item).unwrap() {
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
        world.insert_non_send_resource(lua);
    });
}
