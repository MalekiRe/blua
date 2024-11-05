use crate::userdata_stuff::UserDataPtr;
use crate::LuaVm;
use bevy::ecs::component::ComponentId;
use bevy::ecs::world::FilteredEntityMut;
use bevy::prelude::*;
use piccolo::{
    Callback, CallbackReturn, Context, FromValue, Function, IntoValue, StashedFunction, Table,
    TypeError, UserData, Value,
};
use std::any::TypeId;
use std::cell::RefCell;

#[derive(Copy, Clone, Debug)]
pub enum ComponentType {
    Ref((ComponentId, TypeId)),
    Mut((ComponentId, TypeId)),
}

pub struct LuaSystem {
    pub lua_func: StashedFunction,
    pub queries: Vec<(
        QueryState<FilteredEntityMut<'static>>,
        Vec<(ComponentId, TypeId)>,
    )>,
}

#[derive(Default, Deref, DerefMut)]
pub struct LuaSystems {
    lua_systems: Vec<LuaSystem>,
}

pub struct ReflectPtr {
    data: RefCell<Option<*mut dyn Reflect>>,
    path: String,
}

impl ReflectPtr {
    pub fn new(reflect: &mut dyn Reflect) -> Self {
        Self {
            data: RefCell::new(Some(reflect as *mut dyn Reflect)),
            path: "".to_string(),
        }
    }
    pub fn take(&self) -> Option<()> {
        self.data.take().map(|_| ())
    }
    pub fn get_field_value_ref(&self) -> &dyn Reflect {
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
        let mut reflect = unsafe { &mut *self.get_data() };
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

    fn get_data(&self) -> *mut Self::Data {
        self.data.borrow().unwrap()
    }

    fn edit_metatable<'gc>(&self, _table: &mut Table<'gc>) {}

    fn lua_to_string(&self) -> String {
        format!("{:?}", self.get_field_value_ref())
    }

    // TODO safe mutability by seperating mut vs ref pointers
    fn lua_index<'gc>(&self, ctx: &Context<'gc>, key: &str) -> Value<'gc> {
        let mut reflect_ptr = self.clone();
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
                for field in reflect_ptr.path.split(".") {
                    reflect_field = reflect_field
                        .as_reflect_mut()
                        .reflect_mut()
                        .as_struct()
                        .unwrap()
                        .field_mut(field)
                        .unwrap()
                        .try_as_reflect_mut()
                        .unwrap();
                }
                reflect_field.set(Box::new(n as f32)).unwrap();
            }
            Value::Integer(i) => {
                let mut reflect_field: &mut dyn Reflect = reflect_ptr.get_field_value_mut();
                for field in reflect_ptr.path.split(".") {
                    reflect_field = reflect_field
                        .as_reflect_mut()
                        .reflect_mut()
                        .as_struct()
                        .unwrap()
                        .field_mut(field)
                        .unwrap()
                        .try_as_reflect_mut()
                        .unwrap();
                }
                reflect_field.set(Box::new(i as i32)).unwrap();
            }
            Value::Boolean(b) => {
                let mut reflect_field: &mut dyn Reflect = reflect_ptr.get_field_value_mut();
                for field in reflect_ptr.path.split(".") {
                    reflect_field = reflect_field
                        .as_reflect_mut()
                        .reflect_mut()
                        .as_struct()
                        .unwrap()
                        .field_mut(field)
                        .unwrap()
                        .try_as_reflect_mut()
                        .unwrap();
                }
                reflect_field.set(Box::new(b)).unwrap();
            }
            Value::String(_s) => {
                todo!()
            }
            Value::UserData(data) => {
                if reflect_ptr.path.is_empty() {
                    reflect_ptr.data = data.downcast_static::<ReflectPtr>().unwrap().data.clone();
                    reflect_ptr.path = String::default();
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
            data: RefCell::new(Some(self.data.borrow_mut().unwrap())),
            path: self.path.clone(),
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

    fn get_data(&self) -> *mut Self::Data {
        self.this.unwrap()
    }

    fn edit_metatable<'gc>(&self, _table: &mut Table<'gc>) {}

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
            let (this, system, system_params): (&WorldMut, Value, Table) = stack.consume(ctx)?;

            let mut lua_systems = unsafe { &mut *this.get_data() }
                .get_non_send_resource_mut::<LuaSystems>()
                .unwrap();

            let function: Function = Function::from_value(ctx, system)?;

            let world = unsafe { &mut *this.get_data() };

            let mut queries = vec![];

            for (_, system_parameter) in system_params.into_iter() {
                let table = Table::from_value(ctx, system_parameter)?;
                let mut query_builder = QueryBuilder::<FilteredEntityMut>::new(world);

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
                            components.push((component_id, type_id));
                        }
                        ComponentType::Mut((component_id, type_id)) => {
                            query_builder.mut_id(component_id);
                            components.push((component_id, type_id));
                        }
                    }
                }
                let query_state = query_builder.build();
                queries.push((query_state, components));
            }

            let stashed_function = ctx.stash(function);

            lua_systems.lua_systems.push(LuaSystem {
                lua_func: stashed_function,
                queries,
            });

            Ok(CallbackReturn::Return)
        })
    }
}

pub struct ReflectPlugin;

impl Plugin for ReflectPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(LuaSystems::default());
        app.add_systems(PostStartup, register_components);
    }
}

fn register_components(world: &mut World) {
    world.resource_scope(|world, registry: Mut<AppTypeRegistry>| {
        let mut lua = world.remove_non_send_resource::<LuaVm>().unwrap();
        for item in registry.read().iter() {
            let Some(component_id) = world.components().get_id(item.type_id()) else {
                continue;
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
                        let t = Table::new(&ctx);

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

                        lua_table.set(ctx, item, t).unwrap();
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
        world.insert_non_send_resource(lua);
    });
}
