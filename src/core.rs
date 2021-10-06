// Copyright (c) 2021 Scott J Maddox
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub(crate) use lasso::Rodeo as Interner;
use std::hash::Hash;

pub(crate) type Map<K, V> = fxhash::FxHashMap<K, V>;

#[macro_export]
macro_rules! map {
    ($($k:expr => $v:expr),* $(,)?) => {
        std::iter::Iterator::collect(std::array::IntoIter::new([$(($k, $v),)*]))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Symbol(pub(crate) lasso::Spur);

////////////
// Syntax //
////////////

/// Expressions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Intrinsic(Intrinsic),
    Call(Symbol),
    Quote(Box<Expr>),
    Compose(Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    Swap,
    Clone,
    Drop,
    Quote,
    Compose,
    Apply,
}

impl Default for Expr {
    fn default() -> Self {
        Expr::Compose(vec![])
    }
}

///////////////
// Semantics //
///////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Quote(Box<Expr>),
}

impl Value {
    fn unquote(self) -> Box<Expr> {
        match self {
            Value::Quote(e) => e,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValueStack(pub(crate) Vec<Value>);

pub struct Context {
    pub(crate) interner: Interner,
    pub(crate) fns: Map<Symbol, Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    TooFewValues { available: usize, expected: usize },
    UndefinedFn(Symbol),
}

impl Default for Context {
    fn default() -> Self {
        let interner = Interner::default();
        Context {
            interner,
            fns: Map::default(),
        }
    }
}

impl Context {
    pub fn small_step(&mut self, vs: &mut ValueStack, e: &mut Expr) -> Result<(), EvalError> {
        match e {
            Expr::Intrinsic(intr) => match intr {
                Intrinsic::Swap => {
                    if vs.0.len() < 2 {
                        Err(EvalError::TooFewValues {
                            available: vs.0.len(),
                            expected: 2,
                        })
                    } else {
                        let v = vs.0.remove(vs.0.len() - 2);
                        vs.0.push(v);
                        *e = Expr::default();
                        Ok(())
                    }
                }
                Intrinsic::Clone => {
                    if vs.0.len() < 1 {
                        Err(EvalError::TooFewValues {
                            available: vs.0.len(),
                            expected: 1,
                        })
                    } else {
                        vs.0.push(vs.0.last().unwrap().clone());
                        *e = Expr::default();
                        Ok(())
                    }
                }
                Intrinsic::Drop => {
                    if vs.0.len() < 1 {
                        Err(EvalError::TooFewValues {
                            available: vs.0.len(),
                            expected: 1,
                        })
                    } else {
                        vs.0.pop();
                        *e = Expr::default();
                        Ok(())
                    }
                }
                Intrinsic::Quote => {
                    if vs.0.len() < 1 {
                        Err(EvalError::TooFewValues {
                            available: vs.0.len(),
                            expected: 1,
                        })
                    } else {
                        let v = vs.0.pop().unwrap();
                        let qe = match v {
                            Value::Quote(e) => Expr::Quote(e),
                        };
                        vs.0.push(Value::Quote(Box::new(qe)));
                        *e = Expr::default();
                        Ok(())
                    }
                }
                Intrinsic::Compose => {
                    if vs.0.len() < 2 {
                        Err(EvalError::TooFewValues {
                            available: vs.0.len(),
                            expected: 2,
                        })
                    } else {
                        let e2 = vs.0.pop().unwrap().unquote();
                        let e1 = vs.0.pop().unwrap().unquote();
                        let mut new_es = match (*e1, *e2) {
                            (Expr::Compose(mut e1s), Expr::Compose(mut e2s)) => {
                                e1s.extend(e2s.drain(..));
                                e1s
                            }
                            (Expr::Compose(mut e1s), e2) => {
                                e1s.push(e2);
                                e1s
                            }
                            (e1, Expr::Compose(mut e2s)) => {
                                e2s.insert(0, e1);
                                e2s
                            }
                            (e1, e2) => vec![e1, e2],
                        };
                        let new_e = if new_es.len() == 1 {
                            new_es.drain(..).next().unwrap()
                        } else {
                            Expr::Compose(new_es)
                        };
                        vs.0.push(Value::Quote(Box::new(new_e)));
                        *e = Expr::default();
                        Ok(())
                    }
                }
                Intrinsic::Apply => {
                    if vs.0.len() < 1 {
                        Err(EvalError::TooFewValues {
                            available: vs.0.len(),
                            expected: 1,
                        })
                    } else {
                        let e1 = vs.0.pop().unwrap().unquote();
                        *e = *e1;
                        Ok(())
                    }
                }
            },
            Expr::Call(sym) => {
                if let Some(new_e) = self.fns.get(sym) {
                    *e = new_e.clone();
                    Ok(())
                } else {
                    Err(EvalError::UndefinedFn(*sym))
                }
            }
            Expr::Quote(qe) => {
                vs.0.push(Value::Quote(qe.clone()));
                *e = Expr::default();
                Ok(())
            }
            Expr::Compose(ref mut es) => {
                let es_len = es.len();
                if es_len == 0 {
                    Ok(())
                } else {
                    let e1 = es.first_mut().unwrap();
                    self.small_step(vs, e1)?;
                    match e1 {
                        Expr::Compose(e1s) => {
                            let mut new_es = Vec::with_capacity(e1s.len() + es_len - 1);
                            new_es.extend(e1s.drain(..));
                            new_es.extend(es.drain(1..));
                            let new_e = if new_es.len() == 1 {
                                new_es.drain(..).next().unwrap()
                            } else {
                                Expr::Compose(new_es)
                            };
                            *e = new_e;
                        }
                        _ => {}
                    }
                    Ok(())
                }
            }
        }
    }

    pub fn eval(&mut self, vs: &mut ValueStack, e: &mut Expr) -> Result<(), EvalError> {
        while e != &Expr::default() {
            self.small_step(vs, e)?;
        }
        Ok(())
    }
}

//////////////////////////
// Function Definitions //
//////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDef(pub Symbol, pub Expr);

impl Context {
    pub fn define_fn(&mut self, fn_def: FnDef) -> Option<FnDef> {
        let result = self.fns.remove(&fn_def.0).map(|e| FnDef(fn_def.0, e));
        self.fns.insert(fn_def.0, fn_def.1);
        result
    }
}
