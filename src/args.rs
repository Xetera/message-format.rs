// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::Value;

use std::collections::HashMap;

pub trait Args<'a> {
    fn get(&self, name: &str) -> Option<&'a Value>;
}

pub struct EmptyArgs;

impl<'a> Args<'a> for EmptyArgs {
    fn get(&self, _name: &str) -> Option<&'a Value> { None }
}

impl<'a> Args<'a> for HashMap<&str, Value<'a>> {
    fn get(&self, name: &str) -> Option<&'a Value> {
        self.get(name)
    }
}

/// Holds the arguments being used to format a [`Message`].
///
/// This is a linked list. This avoids any allocations for a `Vec`
/// or `HashMap`. There won't be enough arguments to most messages
/// to make doing linear searches on the arguments costly enough
/// to matter.
///
/// [`Message`]: struct.Message.html
pub struct ListArgs<'a> {
    /// The name of the argument which must match the usage within
    /// the message text.
    pub name: &'a str,
    /// The value of the argument.
    pub value: Value<'a>,
    /// The 'next' argument (which is really the previous since this
    /// is a linked list with the last argument first).
    pub prev: Option<&'a ListArgs<'a>>,
}

/// Create an argument holder.
///
/// This isn't commonly used as arguments are usually set up via the
/// `format_message!` or `write_message!` macros.
///
/// ```
/// use message_format::{ Args, arg };
///
/// let args = arg("name", "John");
/// assert!(args.get("name").is_some());
/// ```
pub fn arg<'a, T: 'a>(name: &'a str, value: T) -> ListArgs<'a>
where
    Value<'a>: From<T>,
{
    ListArgs {
        name: name,
        value: Value::from(value),
        prev: None,
    }
}

impl<'a> ListArgs<'a> {
    /// Add an additional argument. This returns a new value which maintains a link
    /// to the old value. You must maintain a reference to the return value for it to
    /// remain valid.
    ///
    /// This isn't commonly used as arguments are usually set up via the
    /// `format_message!` or `write_message!` macros.
    ///
    /// ```
    /// use message_format::{ Args, arg };
    ///
    /// let args = arg("name", "John");
    /// let args = args.arg("city", "Rome");
    /// assert!(args.get("name").is_some());
    /// assert!(args.get("city").is_some());
    /// ```
    pub fn arg<T: 'a>(&'a self, name: &'a str, value: T) -> ListArgs<'a>
    where
        Self: Sized,
        Value<'a>: From<T>,
    {
        ListArgs {
            name: name,
            value: Value::from(value),
            prev: Some(self),
        }
    }
}

impl<'a> Args<'a> for ListArgs<'a> {

    /// Retrieve the argument with the given `name`.
    ///
    /// ```
    /// use message_format::{ Args, arg };
    ///
    /// let args = arg("count", 3);
    /// let arg = args.get("count").unwrap();
    /// ```
    fn get(&self, name: &str) -> Option<&'a Value> {
        if self.name == name {
            Some(&self.value)
        } else if let Some(prev) = self.prev {
            prev.get(name)
        } else {
            None
        }
    }

    // fn value(&'a self) -> &'a Value<'a> {
    //     &self.value
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Value;

    #[test]
    fn get_works() {
        use super::Args;

        let name = "John";
        let args = arg("name", name);
        assert_eq!(format!("{}", args.get("name").unwrap()), "John");
    }

    #[test]
    fn numbers_work() {
        use super::Args;

        let count = 3;
        let args = arg("count", count);
        assert_eq!(args.get("count").unwrap(), &Value::Number(3));
        assert_eq!(format!("{}", args.get("count").unwrap()), "3");
    }
}
