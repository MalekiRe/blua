use piccolo::{
    Callback, CallbackReturn, Context, FromValue, IntoValue, Table, TypeError, UserData, Value,
};

pub trait UserDataPtr: Sized + 'static
where
    for<'gc> &'gc Self: FromValue<'gc>,
{
    type Data: ?Sized;

    fn get_data_mut(&self) -> Option<*mut Self::Data>;

    fn get_data(&self) -> *const Self::Data;

    fn into_value<'gc>(self, ctx: &Context<'gc>) -> Value<'gc> {
        let metatable = self.metatable(ctx);
        let userdata = UserData::new_static(ctx, self);
        userdata.set_metatable(ctx, Some(metatable));
        userdata.into()
    }

    fn metatable<'gc>(&self, ctx: &Context<'gc>) -> Table<'gc> {
        let mut metatable = Table::new(ctx);

        metatable
            .set(
                *ctx,
                "__tostring",
                Callback::from_fn(ctx, move |ctx, _fuel, mut stack| {
                    let this: &Self = stack.consume(ctx)?;
                    let s = this.lua_to_string();
                    let v = Value::String(piccolo::String::from_slice(&ctx, &s));
                    stack.push_front(v);
                    Ok(CallbackReturn::Return)
                }),
            )
            .unwrap();

        metatable
            .set(
                *ctx,
                "__index",
                Callback::from_fn(ctx, move |ctx, _fuel, mut stack| {
                    let (this, key): (&Self, Value) = stack.consume(ctx)?;
                    let s = key.into_string(ctx).unwrap().to_str().unwrap();
                    stack.push_front(this.lua_index(&ctx, s));

                    Ok(CallbackReturn::Return)
                }),
            )
            .unwrap();

        metatable
            .set(
                *ctx,
                "__newindex",
                Callback::from_fn(ctx, move |ctx, _fuel, mut stack| {
                    let (this, key, new_value): (&Self, Value, Value) = stack.consume(ctx)?;
                    let s = key.into_string(ctx).unwrap().to_str().unwrap();
                    this.lua_new_index(&ctx, s, new_value);

                    Ok(CallbackReturn::Return)
                }),
            )
            .unwrap();

        self.edit_metatable(ctx, &mut metatable);

        metatable
    }

    fn edit_metatable<'gc>(&self, ctx: &Context<'gc>, table: &mut Table<'gc>);

    fn lua_to_string(&self) -> String;

    fn lua_index<'gc>(&self, ctx: &Context<'gc>, key: &str) -> Value<'gc>;

    fn lua_new_index<'gc>(&self, ctx: &Context<'gc>, key: &str, new_value: Value<'gc>);

    fn from_value_2<'gc>(_ctx: Context<'gc>, value: Value<'gc>) -> Result<&'gc Self, TypeError> {
        value.as_static_user_data::<Self>()
    }
}

pub trait ValueExt<'gc> {
    /// Convert to a static user data type.
    fn as_static_user_data<T: 'static>(&self) -> Result<&'gc T, piccolo::TypeError>;
}
impl<'gc> ValueExt<'gc> for Value<'gc> {
    fn as_static_user_data<T: 'static>(&self) -> Result<&'gc T, piccolo::TypeError> {
        if let Value::UserData(t) = self {
            Ok(t.downcast_static().map_err(|_| piccolo::TypeError {
                expected: std::any::type_name::<T>(),
                found: "other user data",
            })?)
        } else {
            Err(piccolo::TypeError {
                expected: std::any::type_name::<T>(),
                found: "other lua value",
            })
        }
    }
}
