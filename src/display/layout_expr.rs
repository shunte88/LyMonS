/*
 *  display/layout_expr.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Simple arithmetic expression evaluator for layout position/size strings.
 *
 *  Supports:
 *    - Integer literals                       42
 *    - Named variables                        display.width  parent.height
 *    - Field geometry references              status_bar.bottom  album.right
 *    - Binary operators                       +  -  *  /
 *    - Parentheses                            (display.width - parent.width) / 2
 *    - Unary negation                         -4
 *
 *  All values are i32.  Division truncates toward zero.
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

#![allow(dead_code)]

use std::collections::HashMap;

/// Context passed to `eval`.
///
/// `display` and `parent` are fixed for a given instantiation.
/// `fields` accumulates resolved field geometry as each field is processed.
pub struct ExprContext<'a> {
    pub display_width:  i32,
    pub display_height: i32,
    pub parent_width:   i32,
    pub parent_height:  i32,
    /// Geometry of already-resolved fields: name → (x, y, w, h).
    pub fields: &'a HashMap<String, FieldGeom>,
    /// Current font height (0 when no font is set).
    pub font_height: i32,
}

/// Resolved pixel geometry of a single field.
#[derive(Debug, Clone, Copy)]
pub struct FieldGeom {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl FieldGeom {
    pub fn top(&self)    -> i32 { self.y }
    pub fn bottom(&self) -> i32 { self.y + self.h }
    pub fn left(&self)   -> i32 { self.x }
    pub fn right(&self)  -> i32 { self.x + self.w }
    pub fn width(&self)  -> i32 { self.w }
    pub fn height(&self) -> i32 { self.h }
}

/// Evaluate an expression string in the given context.
///
/// Returns `Err` with a description if the expression is malformed or a
/// variable is undefined.
pub fn eval(expr: &str, ctx: &ExprContext<'_>) -> Result<i32, String> {
    let tokens = tokenize(expr.trim())?;
    let mut pos = 0usize;
    let result = parse_expr(&tokens, &mut pos, ctx)?;
    if pos != tokens.len() {
        return Err(format!("unexpected token at position {} in '{}'", pos, expr));
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(i32),
    Ident(String),   // display.width, parent.height, status_bar.bottom, …
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => { i += 1; }
            '+' => { tokens.push(Token::Plus);   i += 1; }
            '-' => { tokens.push(Token::Minus);  i += 1; }
            '*' => { tokens.push(Token::Star);   i += 1; }
            '/' => { tokens.push(Token::Slash);  i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::Number(s.parse().map_err(|_| format!("bad number '{}'", s))?));
            }
            c if c.is_alphabetic() || c == '_' => {
                // Identifiers may contain letters, digits, underscores, and dots
                // (e.g. `display.width`, `status_bar.bottom`)
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                tokens.push(Token::Ident(chars[start..i].iter().collect()));
            }
            c => return Err(format!("unexpected character '{}'", c)),
        }
    }
    Ok(tokens)
}

// Recursive descent parser (precedence climbing) 
//
//  expr   = term   { ('+' | '-') term }
//  term   = factor { ('*' | '/') factor }
//  factor = '-' factor | '(' expr ')' | atom
//  atom   = Number | Ident

fn parse_expr(tokens: &[Token], pos: &mut usize, ctx: &ExprContext<'_>) -> Result<i32, String> {
    let mut val = parse_term(tokens, pos, ctx)?;
    loop {
        match tokens.get(*pos) {
            Some(Token::Plus)  => { *pos += 1; val += parse_term(tokens, pos, ctx)?; }
            Some(Token::Minus) => { *pos += 1; val -= parse_term(tokens, pos, ctx)?; }
            _ => break,
        }
    }
    Ok(val)
}

fn parse_term(tokens: &[Token], pos: &mut usize, ctx: &ExprContext<'_>) -> Result<i32, String> {
    let mut val = parse_factor(tokens, pos, ctx)?;
    loop {
        match tokens.get(*pos) {
            Some(Token::Star) => {
                *pos += 1;
                val *= parse_factor(tokens, pos, ctx)?;
            }
            Some(Token::Slash) => {
                *pos += 1;
                let rhs = parse_factor(tokens, pos, ctx)?;
                if rhs == 0 { return Err("division by zero".to_string()); }
                val /= rhs;
            }
            _ => break,
        }
    }
    Ok(val)
}

fn parse_factor(tokens: &[Token], pos: &mut usize, ctx: &ExprContext<'_>) -> Result<i32, String> {
    match tokens.get(*pos) {
        Some(Token::Minus) => {
            *pos += 1;
            Ok(-parse_factor(tokens, pos, ctx)?)
        }
        Some(Token::LParen) => {
            *pos += 1;
            let val = parse_expr(tokens, pos, ctx)?;
            match tokens.get(*pos) {
                Some(Token::RParen) => { *pos += 1; Ok(val) }
                _ => Err("expected closing ')'".to_string()),
            }
        }
        _ => parse_atom(tokens, pos, ctx),
    }
}

fn parse_atom(tokens: &[Token], pos: &mut usize, ctx: &ExprContext<'_>) -> Result<i32, String> {
    match tokens.get(*pos) {
        Some(Token::Number(n)) => { let v = *n; *pos += 1; Ok(v) }
        Some(Token::Ident(name)) => {
            let name = name.clone();
            *pos += 1;
            resolve_ident(&name, ctx)
        }
        other => Err(format!("expected value, got {:?}", other)),
    }
}

// Variable resolution

fn resolve_ident(name: &str, ctx: &ExprContext<'_>) -> Result<i32, String> {
    match name {
        "display.width"  => return Ok(ctx.display_width),
        "display.height" => return Ok(ctx.display_height),
        "parent.width"   => return Ok(ctx.parent_width),
        "parent.height"  => return Ok(ctx.parent_height),
        "font_height"    => return Ok(ctx.font_height),
        _ => {}
    }

    // `<field_name>.<property>`
    if let Some(dot) = name.rfind('.') {
        let field_name = &name[..dot];
        let prop       = &name[dot+1..];
        if let Some(geom) = ctx.fields.get(field_name) {
            return match prop {
                "top"    => Ok(geom.top()),
                "bottom" => Ok(geom.bottom()),
                "left"   => Ok(geom.left()),
                "right"  => Ok(geom.right()),
                "width"  => Ok(geom.width()),
                "height" => Ok(geom.height()),
                other    => Err(format!("unknown property '{}' on field '{}'", other, field_name)),
            };
        }
        return Err(format!("unknown field '{}' — has it been defined above this field?", field_name));
    }

    Err(format!("unknown variable '{}'", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx<'a>(
        dw: i32, dh: i32, pw: i32, ph: i32,
        fields: &'a HashMap<String, FieldGeom>,
    ) -> ExprContext<'a> {
        ExprContext {
            display_width: dw,
            display_height: dh,
            parent_width: pw,
            parent_height: ph,
            fields,
            font_height: 9,
        }
    }

    #[test]
    fn literal() {
        let fields = HashMap::new();
        let ctx = make_ctx(128, 64, 64, 64, &fields);
        assert_eq!(eval("42", &ctx).unwrap(), 42);
    }

    #[test]
    fn display_vars() {
        let fields = HashMap::new();
        let ctx = make_ctx(256, 64, 128, 64, &fields);
        assert_eq!(eval("display.width", &ctx).unwrap(), 256);
        assert_eq!(eval("display.width / 2", &ctx).unwrap(), 128);
        assert_eq!(eval("display.height - 10", &ctx).unwrap(), 54);
    }

    #[test]
    fn parent_vars() {
        let fields = HashMap::new();
        let ctx = make_ctx(128, 64, 64, 64, &fields);
        assert_eq!(eval("parent.width", &ctx).unwrap(), 64);
        assert_eq!(eval("parent.height - 14", &ctx).unwrap(), 50);
    }

    #[test]
    fn field_refs() {
        let mut fields = HashMap::new();
        fields.insert("status_bar".to_string(), FieldGeom { x: 0, y: 0, w: 128, h: 9 });
        let ctx = make_ctx(128, 64, 128, 64, &fields);
        assert_eq!(eval("status_bar.bottom", &ctx).unwrap(), 9);
        assert_eq!(eval("status_bar.bottom + 1", &ctx).unwrap(), 10);
        assert_eq!(eval("status_bar.right", &ctx).unwrap(), 128);
    }

    #[test]
    fn precedence_and_parens() {
        let fields = HashMap::new();
        let ctx = make_ctx(320, 170, 150, 170, &fields);
        assert_eq!(eval("2 + 3 * 4", &ctx).unwrap(), 14);
        assert_eq!(eval("(2 + 3) * 4", &ctx).unwrap(), 20);
        assert_eq!(eval("display.width - display.height", &ctx).unwrap(), 150);
    }

    #[test]
    fn unary_minus() {
        let fields = HashMap::new();
        let ctx = make_ctx(128, 64, 128, 64, &fields);
        assert_eq!(eval("-4", &ctx).unwrap(), -4);
        assert_eq!(eval("display.height + -4", &ctx).unwrap(), 60);
    }

    #[test]
    fn div_zero_err() {
        let fields = HashMap::new();
        let ctx = make_ctx(128, 64, 128, 64, &fields);
        assert!(eval("display.width / 0", &ctx).is_err());
    }

    #[test]
    fn unknown_field_err() {
        let fields = HashMap::new();
        let ctx = make_ctx(128, 64, 128, 64, &fields);
        assert!(eval("nope.bottom", &ctx).is_err());
    }
}
