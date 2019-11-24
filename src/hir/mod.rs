pub mod ast2hir;
pub mod find_builtin;
pub mod flat_expr;
pub mod flat_let;
pub mod force_closure;
pub mod pp;
pub mod unnest_func;
pub mod util;

pub use self::ast2hir::AST2HIR;
pub use self::find_builtin::FindBuiltin;
pub use self::flat_expr::FlatExpr;
pub use self::flat_let::FlatLet;
pub use self::force_closure::ForceClosure;
pub use self::unnest_func::UnnestFunc;

use crate::prim::*;

#[derive(Debug)]
pub struct HIR(pub Vec<Val>);

#[derive(Debug, Clone)]
pub struct Val {
    pub ty: HTy,
    pub rec: bool,
    pub name: Symbol,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Binds {
        ty: HTy,
        binds: Vec<Val>,
        ret: Box<Expr>,
    },
    BinOp {
        ty: HTy,
        name: Symbol,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    BuiltinCall {
        ty: HTy,
        fun: BIF,
        arg: Box<Expr>,
    },
    Fun {
        param: (HTy, Symbol),
        body_ty: HTy,
        body: Box<Expr>,
        captures: Vec<(HTy, Symbol)>,
    },
    Closure {
        envs: Vec<(HTy, Symbol)>,
        param_ty: HTy,
        body_ty: HTy,
        fname: Symbol,
    },
    App {
        ty: HTy,
        fun: Box<Expr>,
        arg: Box<Expr>,
    },
    Case {
        ty: HTy,
        expr: Box<Expr>,
        arms: Vec<(Pattern, Expr)>,
    },
    Tuple {
        tys: Vec<HTy>,
        tuple: Vec<Expr>,
    },
    Proj {
        ty: HTy,
        /// 0-origin
        index: u32,
        tuple: Box<Expr>,
    },
    Constructor {
        ty: HTy,
        arg: Option<Box<Expr>>,
        descriminant: u32,
    },
    Sym {
        ty: HTy,
        name: Symbol,
    },
    Lit {
        ty: HTy,
        value: Literal,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Constant {
        value: i64,
        ty: HTy,
    },
    Constructor {
        descriminant: u32,
        arg: Option<(HTy, Symbol)>,
        ty: HTy,
    },
    Var {
        name: Symbol,
        ty: HTy,
    },
    Tuple {
        tys: Vec<HTy>,
        tuple: Vec<Symbol>,
    },
}

impl Pattern {
    pub fn match_key(&self) -> u32 {
        use self::Pattern::*;
        // FIXME do not panic
        match self {
            Constant { value, .. } => *value as u32,
            Tuple { .. } => panic!("bug: non-variant expression does not have keys"),
            Constructor { descriminant, .. } => *descriminant as u32,
            Var { .. } => panic!("bug: default like branch does not have keys"),
        }
    }

    pub fn binds(&self) -> Option<Symbol> {
        use self::Pattern::*;
        // FIXME do not panic
        match self {
            Constant { .. } => None,
            Tuple { .. } => panic!("bug: non-variant expression does not have keys"),
            Constructor { arg, .. } => arg.as_ref().map(|(_, name)| name.clone()),
            Var { name, .. } => Some(name.clone()),
        }
    }

    pub fn is_irrefutable(&self) -> bool {
        use self::Pattern::*;
        match *self {
            Constructor { .. } | Constant { .. } => false,
            Tuple { .. } | Var { .. } => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HTy {
    Int,
    Real,
    Tuple(Vec<HTy>),
    Fun(Box<HTy>, Box<HTy>),
    Datatype(Vec<(u32, Option<HTy>)>),
}

impl Expr {
    fn app1(self, ty: HTy, e: Expr) -> Expr {
        Expr::App {
            ty,
            fun: Box::new(self),
            arg: Box::new(e),
        }
    }

    pub fn ty(&self) -> HTy {
        use crate::hir::Expr::*;

        match self {
            &Closure {
                ref param_ty,
                ref body_ty,
                ..
            }
            | &Fun {
                param: (ref param_ty, _),
                ref body_ty,
                ..
            } => HTy::fun(param_ty.clone(), body_ty.clone()),
            &Tuple { ref tys, .. } => HTy::Tuple(tys.clone()),
            &Proj { ref ty, .. }
            | &BinOp { ref ty, .. }
            | &Binds { ref ty, .. }
            | &BuiltinCall { ref ty, .. }
            | &App { ref ty, .. }
            | &Case { ref ty, .. }
            | &Constructor { ref ty, .. }
            | &Sym { ref ty, .. }
            | &Lit { ref ty, .. } => ty.clone(),
        }
    }
}

impl HTy {
    pub fn fun(arg: HTy, ret: HTy) -> HTy {
        HTy::Fun(Box::new(arg), Box::new(ret))
    }
}
