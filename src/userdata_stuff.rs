use std::cell::RefCell;
use piccolo::{Context, FromValue, Table, TypeError, UserData, Value};

pub struct UserDataWrapper<Data: 'static, Other: Clone + 'static = ()> {
    pub data: RefCell<Option<*mut Data>>,
    pub other: Other,
}

impl<Data, Other: Clone> UserDataWrapper<Data, Other> {
    pub fn new(data: &mut Data, other: Other) -> Self {
        Self {
            data: RefCell::new(Some(data as *mut Data)),
            other,
        }
    }
    pub fn into_value<'gc>(self, ctx: &Context<'gc>, metatable: Table<'gc>) -> Value<'gc> {
        let userdata = UserData::new_static(ctx, self);
        userdata.set_metatable(ctx, Some(metatable));
        userdata.into()
    }
}

impl<Data, Other: Clone> Clone for UserDataWrapper<Data, Other> {
    fn clone(&self) -> Self {
        UserDataWrapper {
            data: RefCell::new(Some(self.data.borrow_mut().unwrap().clone())),
            other: self.other.clone(),
        }
    }
}

impl<'gc, Data: 'static, Other: Clone + 'static> FromValue<'gc> for &'gc UserDataWrapper<Data, Other> {
    fn from_value(ctx: Context<'gc>, value: Value<'gc>) -> Result<Self, TypeError> {
        value.as_static_user_data::<UserDataWrapper<Data, Other>>()
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