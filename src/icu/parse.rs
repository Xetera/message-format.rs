// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::error::Error;
use std::fmt;
use std::str;

use nom::character::complete::{ alphanumeric1, digit1, multispace0 };
use nom::bytes::complete::{ tag, is_not, take_while };
use nom::sequence::delimited;
use nom::{dbg_dmp, IResult};
use nom::combinator::{ opt, map_parser, flat_map, map };
use nom::multi::many1;
use nom::branch::alt;

use super::ast;
use super::ast::PlainText;
use {Message, MessagePart};

/// An error resulting from `parse`.
#[derive(Clone, Debug)]
pub enum ParseError {
    /// The message could not be parsed.
    NotImplemented,
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::NotImplemented => "Not implemented.",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.description().fmt(f)
    }
}

/// Given a name, create a `SimpleFormat`.
fn mk_simple(name: &str) -> Box<dyn MessagePart> {
    Box::new(ast::SimpleFormat::new(name))
}

// This grabs the variable name from a format, which is
// the first thing after the '{' and extends to the first
// ',' or '}'.
//
// '{name}' has a variable name of 'name'.
fn variable_name(s: &str) -> IResult<&str, &str> {
    is_not(",}")(s)
}

// A simple format has only a name, delimited by braces.
pub fn simple_format(s: &str) -> IResult<&str, Box<dyn MessagePart>> {
    map(
        delimited(
            tag("{"),
            variable_name,
            tag("}")
        ),
        mk_simple
    )(s)
}

fn submessage(s: &str) -> IResult<&str, Message> {
    delimited(
        tag("{"),
        map_parser(is_not("}"), message_parser),
        tag("}")
    )(s)
}

fn plural_literal(s: &str) -> IResult<&str, PluralPart> {
    do_parse!(s,
        call!(tag("="))             >>
        offset: call!(digit1)       >>
        call!(opt(multispace0))     >>
        msg: call!(submessage)      >>
        multispace0                 >>
        (PluralPart::Literal(offset.parse().unwrap(), msg))
    )
}

//one {1 day}
fn plural_one(s: &str) -> IResult<&str,PluralPart> {
    do_parse!(s,
        multispace0             >>
        tag!("one")             >>
        multispace0             >>
        msg: call!(submessage)  >>
        multispace0             >>
        (PluralPart::One(msg))
    )
}

fn plural_other(s: &str) -> IResult<&str,PluralPart> {
    do_parse!(s,
        multispace0                 >>
        tag!("other")               >>
        multispace0                 >>
        msg: call!(submessage)      >>
        multispace0                 >>
        (PluralPart::Other(msg))
    )
}
#[derive(Debug)]
enum PluralPart {
    Literal(i64, Message),
    Zero(Message),
    One(Message),
    Two(Message),
    Few(Message),
    Many(Message),
    Other(Message),
}

fn plural_from_parts(var_name: &str, mut parts: Vec<PluralPart>) -> ast::PluralFormat {
    // println!("parts = {:?}", parts);
    let other_part_pos = parts.iter().position(|pp| {
        match pp {
            PluralPart::Other(_) => true,
            _ => false
        }
    });

    let mut fmt = if let Some(other_part_pos) = other_part_pos {
        let other_part = match parts.remove(other_part_pos) {
            PluralPart::Other(m) => m,
            _ => panic!("unreachable")
        };

        ast::PluralFormat::new(var_name, other_part)
    } else {
        panic!("no other part contained in plural")
    };

    for part in parts {
        match part {
            PluralPart::Zero(m) => fmt.zero(m),
            PluralPart::One(m) => fmt.one(m),
            PluralPart::Two(m) => fmt.two(m),
            PluralPart::Few(m) => fmt.few(m),
            PluralPart::Many(m) => fmt.many(m),
            PluralPart::Literal(c,m) => fmt.literal(c,m),
            PluralPart::Other(_) => (), //already added in constructor
        }
    }

    fmt
}

named!(plural_submessage <&str, Vec<PluralPart>>,
    many1!(
        alt!(
            call!(plural_literal) |
            call!(plural_one)     |
            call!(plural_other)
        )
    )
);

fn plural_inner(s: &str) -> IResult<&str, Box<dyn MessagePart>> {
    do_parse!(s,
        name: variable_name             >>
        call!(tag(","))                 >>
        call!(opt(multispace0))         >>
        call!(tag("plural"))            >>
        call!(opt(multispace0))         >>
        call!(tag(","))                 >>
        call!(opt(multispace0))         >>
        parts: call!(plural_submessage) >>
        (Box::new(plural_from_parts(name, parts)) as Box<dyn MessagePart>)
    )
}
//{number, plural, one {1 day} other {# days}}
fn plural_format(s: &str) -> IResult<&str,Box<dyn MessagePart>> {
    delimited(
        tag("{"),
        plural_inner,
        tag("}"),
    )(s)
}

fn select_match(s: &str) -> IResult<&str, (&str, Message)> {
    do_parse!(s,
        multispace0                 >>
        match_cond: alphanumeric1   >>
        multispace0                 >>
        msg: call!(submessage)      >>
        multispace0                 >>
        ((match_cond,msg))
    )
}

fn select_submessage(s: &str) -> IResult<&str, Vec<(&str, Message)>> {
    many1(select_match)(s)
}

fn select_inner(s: &str) -> IResult<&str, Box<dyn MessagePart>> {
    do_parse!(s,
        name: variable_name             >>
        call!(tag(","))                 >>
        call!(opt(multispace0))         >>
        call!(tag("select"))            >>
        call!(opt(multispace0))         >>
        call!(tag(","))                 >>
        call!(opt(multispace0))         >>
        parts: call!(select_submessage) >>
        (Box::new(select_from_parts(name, parts)) as Box<dyn MessagePart>)
    )
}

fn select_from_parts(variable_name: &str, mut parts: Vec<(&str, Message)>) -> ast::SelectFormat {
    let other_part_pos = parts.iter().position(|(n,_)| *n == "other");

    if let Some(other_part_pos) = other_part_pos {
        let (_,other_part) = parts.remove(other_part_pos);
        let mut fmt = ast::SelectFormat::new(variable_name, other_part);

        for (s,p) in parts {
            fmt.map(s, p);
        }

        fmt
    } else {
        panic!("no other part found for select")
    }
}

fn select_format(s: &str) -> IResult<&str, Box<dyn MessagePart>> {
    delimited(
        tag("{"),
        select_inner,
        tag("}"),
    )(s)
}

fn plain_text(s: &str) -> IResult<&str, Box<dyn MessagePart> > {
    map(
        is_not("{#"),
        |text| Box::new(ast::PlainText::new(text)) as Box<dyn MessagePart>,
    )(s)
}

fn placeholder(s: &str) -> IResult<&str, Box<dyn MessagePart>> {
    map(
        tag("#"),
        |_| Box::new(ast::PlaceholderFormat::new()) as Box<dyn MessagePart>,
    )(s)
}

pub fn message_parts(s: &str) -> IResult<&str,Vec<Box<dyn MessagePart>>> {
    many1(
        alt((
            placeholder,
            simple_format,
            plural_format,
            select_format,
            plain_text,
        ))
    )(s)
}

// Given a set of `MessagePart`s, create a `Message`.
pub fn message_parser(s: &str) -> IResult<&str, Message> {
    map(message_parts, Message::new)(s)
}

/// Parse some text and hopefully return a [`Message`].
///
/// [`Message`]: ../struct.Message.html
pub fn parse(message: &str) -> Result<Message, ParseError> {
    match message_parser(message) {
        Err(_) => Err(ParseError::NotImplemented),
        Ok((_, m)) => Ok(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use {arg, Context};

    #[test]
    fn plain_text_test() {
        let r = plain_text("hello {name}");

        match r {
            Ok((rem, pt)) => {
                assert_eq!(rem, "{name}");
                // assert_eq!(pt, ast::PlainText::new("hello "));
            },
            Err(err) => panic!("parse error: {:?}", err),
        }
    }
    #[test]
    fn it_works() {
        let ctx = Context::default();
        match parse("{name} is from {city}.") {
            Ok(m) => {
                assert_eq!(
                    ctx.format(&m, &arg("name", "Hendrik").arg("city", "Berlin")),
                    "Hendrik is from Berlin."
                );
            }
            Err(e) => panic!("Parse failed: {}", e),
        }
    }

    // #[test]
    // fn incomplete_fails() {
    //     match message_parser("{name") {
    //         IResult::Incomplete(_) => {}
    //         IResult::Error(e) => panic!("Expected incomplete failure: Got {}", e),
    //         IResult::Done(_, _) => panic!("Expected incomplete failure, but succeeded."),
    //     }
    // }

    #[test]
    fn all_text_works() {
        match message_parser("Hello, world!") {
            Ok((_,_)) => {}
            Err(err) => panic!("Expected successful parse. {:?}", err),
        }
    }

    #[test]
    fn plural_format_works() {
        match message_parser("hello {name} you have {number, plural, =54 {perfect number of days} one {1 day} other {# days}} left") {
            Ok((_, fmt)) => {
                println!("fmt = {:?}", fmt);
                let ctx = Context::default();
                let out = ctx.format(&fmt, &arg("number", 225).arg("name", "Zack"));
                println!("out = {}", out);
            }
            Err(err) => {
                panic!("Parse Err {:?}", err)
            }
        }
    }

    #[test]
    fn select_format_works() {
        match message_parser("{gender, select, male {He} female {She} other {They}} will respond shortly.") {
            Ok((_, fmt)) => {
                println!("fmt = {:?}", fmt);
                let ctx = Context::default();
                let out = ctx.format(&fmt, &arg("gender", "female"));
                println!("out = {}", out);
            }
            _ => panic!("Expected successful parse."),
        }
    }
}
