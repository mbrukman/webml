use crate::ast::*;
use crate::config::Config;
use crate::id::Id;
use crate::prim::*;
use crate::unification_pool::{NodeId, UnificationPool};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Typer;

#[derive(Debug)]
struct TyEnv {
    env: HashMap<Symbol, NodeId>,
    symbol_table: SymbolTable,
    pool: TypePool,
}

#[derive(Debug)]
struct TypePool {
    cache: HashMap<Typing, NodeId>,
    pool: UnificationPool<Typing>,
    id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Typing {
    Variable(u64),
    Char,
    Int,
    Real,
    Fun(NodeId, NodeId),
    Tuple(Vec<NodeId>),
    Datatype(Symbol),
    OverloadedNum,
    OverloadedNumText,
}

fn resolve(pool: &UnificationPool<Typing>, id: NodeId) -> Type {
    conv_ty(pool, pool.value_of(id).clone())
}

fn conv_ty(pool: &UnificationPool<Typing>, ty: Typing) -> Type {
    use Typing::*;
    match ty {
        Variable(id) => Type::Variable(id),
        Char => Type::Char,
        Int => Type::Int,
        Real => Type::Real,
        Fun(param, body) => Type::Fun(
            Box::new(resolve(pool, param)),
            Box::new(resolve(pool, body)),
        ),
        Tuple(tys) => Type::Tuple(tys.into_iter().map(|ty| resolve(pool, ty)).collect()),
        Datatype(type_id) => Type::Datatype(type_id),
        OverloadedNum => Type::Int,
        OverloadedNumText => Type::Int,
    }
}

fn try_unify<'b, 'r>(
    pool: &'b mut UnificationPool<Typing>,
    t1: Typing,
    t2: Typing,
) -> Result<'r, Typing> {
    use Typing::*;
    match (t1, t2) {
        (t1, t2) if t1 == t2 => Ok(t1),
        (Int, OverloadedNum) | (OverloadedNum, Int) => Ok(Int),
        (Int, OverloadedNumText) | (OverloadedNumText, Int) => Ok(Int),
        (Char, OverloadedNumText) | (OverloadedNumText, Char) => Ok(Char),
        (Real, OverloadedNum) | (OverloadedNum, Real) => Ok(Real),
        (Real, OverloadedNumText) | (OverloadedNumText, Real) => Ok(Real),
        (OverloadedNumText, OverloadedNum) | (OverloadedNum, OverloadedNumText) => {
            Ok(OverloadedNumText)
        }
        (Variable(_), ty) | (ty, Variable(_)) => Ok(ty),
        (Fun(p1, b1), Fun(p2, b2)) => {
            let p = pool.try_unify_with(p1, p2, try_unify)?;
            let b = pool.try_unify_with(b1, b2, try_unify)?;
            Ok(Fun(p, b))
        }
        (Tuple(tu1), Tuple(tu2)) => {
            if tu1.len() != tu2.len() {
                Err(TypeError::MisMatch {
                    expected: conv_ty(pool, Tuple(tu1)),
                    actual: conv_ty(pool, Tuple(tu2)),
                })
            } else {
                let tu = tu1
                    .into_iter()
                    .zip(tu2)
                    .map(|(t1, t2)| pool.try_unify_with(t1, t2, try_unify))
                    .collect::<Result<'_, Vec<_>>>()?;
                Ok(Tuple(tu))
            }
        }
        (t1, t2) => Err(TypeError::MisMatch {
            expected: conv_ty(pool, t1),
            actual: conv_ty(pool, t2),
        }),
    }
}

impl Typer {
    pub fn new() -> Self {
        Typer
    }

    fn generate_pass(&mut self, symbol_table: SymbolTable) -> TyEnv {
        TyEnv::new(symbol_table)
    }
}

impl TypePool {
    fn new() -> Self {
        let mut ret = Self {
            cache: HashMap::new(),
            pool: UnificationPool::new(),
            id: Id::new(),
        };
        ret.init();
        ret
    }

    fn init(&mut self) {
        self.node_new(Typing::Char);
        self.node_new(Typing::Int);
        self.node_new(Typing::Real);
    }

    fn feed_symbol_table(&mut self, symbol_table: &SymbolTable) {
        for typename in symbol_table.types.keys() {
            self.node_new(Typing::Datatype(typename.clone()));
        }
    }

    fn tyvar(&mut self) -> NodeId {
        self.pool.node_new(Typing::Variable(self.id.next()))
    }

    fn ty(&mut self, ty: Typing) -> NodeId {
        self.pool.node_new(ty)
    }

    fn ty_int(&mut self) -> NodeId {
        *self.cache.get(&Typing::Int).unwrap()
    }

    fn ty_char(&mut self) -> NodeId {
        *self.cache.get(&Typing::Char).unwrap()
    }

    fn ty_bool(&mut self) -> NodeId {
        *self
            .cache
            .get(&Typing::Datatype(Symbol::new("bool")))
            .unwrap()
    }

    fn ty_real(&mut self) -> NodeId {
        *self.cache.get(&Typing::Real).unwrap()
    }

    fn ty_overloaded_num(&mut self) -> NodeId {
        self.node_new(Typing::OverloadedNum)
    }

    fn ty_overloaded_num_text(&mut self) -> NodeId {
        self.node_new(Typing::OverloadedNumText)
    }

    fn node_new(&mut self, t: Typing) -> NodeId {
        let node_id = self.pool.node_new(t.clone());
        match t {
            t @ Typing::Char | t @ Typing::Int | t @ Typing::Real | t @ Typing::Datatype(_) => {
                self.cache.insert(t, node_id);
            }
            _ => (), // no cache
        }
        node_id
    }

    fn try_unify_with<'r>(
        &mut self,
        id1: NodeId,
        id2: NodeId,
        try_unify: impl FnOnce(&mut UnificationPool<Typing>, Typing, Typing) -> Result<'r, Typing>,
    ) -> Result<'r, NodeId> {
        self.pool.try_unify_with(id1, id2, try_unify)
    }
}

impl TypePool {
    fn typing_ast(&mut self, ast: UntypedCore) -> Core<NodeId> {
        ast.map_ty(&mut |_| self.tyvar())
    }
}

impl TypePool {
    fn typed_ast(&self, ast: Core<NodeId>) -> TypedCore {
        ast.map_ty(&mut |ty| resolve(&self.pool, ty))
    }
}

impl TyEnv {
    pub fn new(symbol_table: SymbolTable) -> Self {
        let mut ret = TyEnv {
            env: HashMap::new(),
            symbol_table: symbol_table,
            pool: TypePool::new(),
        };
        ret.init();

        ret
    }

    fn init(&mut self) {
        self.pool.feed_symbol_table(&self.symbol_table);
        let cnames = self
            .symbol_table
            .constructors
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for cname in cnames {
            let ty = self
                .symbol_table
                .get_datatype_of_constructor(&cname)
                .expect("internal error: typing");
            let ty = Type::Datatype(ty.clone());
            let typing = self.convert(ty);
            let node_id = self.pool.ty(typing);
            self.insert(cname, node_id);
        }
    }

    pub fn infer<'a, 'b>(&'a mut self, ast: &mut ast::Core<NodeId>) -> Result<'b, ()> {
        self.infer_ast(ast)?;
        Ok(())
    }

    fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    pub fn into_symbol_table(self) -> SymbolTable {
        self.symbol_table
    }

    fn get(&self, name: &Symbol) -> Option<NodeId> {
        self.env.get(name).cloned()
    }

    fn insert(&mut self, k: Symbol, v: NodeId) -> Option<NodeId> {
        self.env.insert(k, v)
    }

    fn convert(&mut self, ty: Type) -> Typing {
        match ty {
            Type::Variable(v) => Typing::Variable(v),
            Type::Char => Typing::Char,
            Type::Int => Typing::Int,
            Type::Real => Typing::Real,
            Type::Fun(arg, ret) => {
                let arg_typing = self.convert(*arg);
                let ret_typing = self.convert(*ret);
                Typing::Fun(self.pool.ty(arg_typing), self.pool.ty(ret_typing))
            }
            Type::Tuple(tuple) => Typing::Tuple(
                tuple
                    .into_iter()
                    .map(|ty| {
                        let typing = self.convert(ty);
                        self.pool.ty(typing)
                    })
                    .collect(),
            ),
            Type::Datatype(name) => Typing::Datatype(name),
        }
    }
}

impl TyEnv {
    fn infer_ast<'b, 'r>(&'b mut self, ast: &Core<NodeId>) -> Result<'r, ()> {
        for decl in ast.0.iter() {
            self.infer_statement(&decl)?;
        }
        Ok(())
    }

    fn infer_statement<'b, 'r>(&'b mut self, decl: &CoreDeclaration<NodeId>) -> Result<'r, ()> {
        use Declaration::*;
        match decl {
            Datatype { .. } => Ok(()),
            Val { rec, pattern, expr } => {
                let names = pattern.binds();
                if *rec {
                    for &(name, ty) in &names {
                        self.insert(name.clone(), ty.clone());
                    }
                }
                self.infer_expr(expr)?;
                self.infer_pat(pattern)?;
                self.unify(expr.ty(), pattern.ty())?;
                if !rec {
                    for &(name, ty) in &names {
                        self.insert(name.clone(), ty.clone());
                    }
                }
                Ok(())
            }
            D(d) => match *d {},
        }
    }

    fn infer_expr<'b, 'r>(&'b mut self, expr: &CoreExpr<NodeId>) -> Result<'r, ()> {
        use crate::ast::ExprKind::*;
        let int = self.pool.ty_int();
        let real = self.pool.ty_real();
        let bool = self.pool.ty_bool();
        let overloaded_num = self.pool.ty_overloaded_num();
        let overloaded_num_text = self.pool.ty_overloaded_num_text();
        let ty = &expr.ty;
        match &expr.inner {
            Binds { binds, ret } => {
                for decl in binds {
                    self.infer_statement(decl)?;
                }
                self.unify(ret.ty(), *ty)?;
                self.infer_expr(ret)?;
                Ok(())
            }
            BuiltinCall { fun, args } => {
                use BIF::*;
                match fun {
                    Add | Sub | Mul => {
                        assert!(args.len() == 2);
                        let l = &args[0];
                        let r = &args[1];

                        self.infer_expr(l)?;
                        self.infer_expr(r)?;
                        self.unify(l.ty(), r.ty())?;
                        self.unify(l.ty(), overloaded_num)?;
                        self.unify(*ty, l.ty())?;
                        Ok(())
                    }
                    Eq | Neq | Gt | Ge | Lt | Le => {
                        assert!(args.len() == 2);
                        let l = &args[0];
                        let r = &args[1];

                        self.infer_expr(l)?;
                        self.infer_expr(r)?;
                        self.unify(l.ty(), r.ty())?;
                        self.unify(l.ty(), overloaded_num_text)?;
                        self.unify(*ty, bool)?;
                        Ok(())
                    }
                    Div | Mod => {
                        assert!(args.len() == 2);
                        let l = &args[0];
                        let r = &args[1];

                        self.unify(l.ty(), int)?;
                        self.unify(r.ty(), int)?;
                        self.unify(*ty, int)?;
                        self.infer_expr(l)?;
                        self.infer_expr(r)?;
                        Ok(())
                    }
                    Divf => {
                        assert!(args.len() == 2);
                        let l = &args[0];
                        let r = &args[1];

                        self.unify(l.ty(), real)?;
                        self.unify(r.ty(), real)?;
                        self.unify(*ty, real)?;
                        self.infer_expr(l)?;
                        self.infer_expr(r)?;
                        Ok(())
                    }
                }
            }
            ExternCall {
                args, argty, retty, ..
            } => {
                for (arg, argty) in args.into_iter().zip(argty) {
                    self.infer_expr(arg)?;
                    let argty = self.convert(argty.clone());
                    self.give(arg.ty(), argty)?;
                }
                let retty = self.convert(retty.clone());
                self.give(*ty, retty)?;
                Ok(())
            }
            Fn { param, body } => {
                let param_ty = self.pool.tyvar();
                self.insert(param.clone(), param_ty);
                self.infer_expr(body)?;
                self.give(*ty, Typing::Fun(param_ty, body.ty()))?;
                Ok(())
            }
            App { fun, arg } => {
                self.infer_expr(fun)?;
                self.infer_expr(arg)?;
                self.give(fun.ty(), Typing::Fun(arg.ty(), *ty))?;
                Ok(())
            }
            Case { cond, clauses } => {
                self.infer_expr(cond)?;
                for (pat, branch) in clauses {
                    self.infer_pat(pat)?;
                    self.unify(pat.ty(), cond.ty())?;
                    self.infer_expr(branch)?;
                    self.unify(branch.ty(), *ty)?;
                }
                Ok(())
            }
            Tuple { tuple } => {
                self.infer_tuple(tuple, *ty)?;
                Ok(())
            }
            Constructor { arg, name } => {
                self.infer_constructor(name, arg, *ty)?;
                Ok(())
            }
            Symbol { name } => {
                self.infer_symbol(name, *ty)?;
                Ok(())
            }
            Literal { value } => {
                self.infer_literal(value, *ty)?;
                Ok(())
            }
            D(d) => match *d {},
        }
    }

    fn infer_constructor<'b, 'r>(
        &'b mut self,
        sym: &Symbol,
        arg: &Option<Box<CoreExpr<NodeId>>>,
        given: NodeId,
    ) -> Result<'r, ()> {
        match self.get(&sym) {
            Some(ty) => {
                self.unify(ty, given)?;
                let arg_ty = self.symbol_table().get_argtype_of_constructor(sym);
                if let (Some(arg), Some(arg_ty)) = (arg.clone(), arg_ty.cloned()) {
                    self.infer_expr(&arg)?;
                    let arg_typing = self.convert(arg_ty);
                    let arg_ty_id = self.pool.ty(arg_typing);
                    self.unify(arg.ty(), arg_ty_id)?;
                }
                Ok(())
            }
            None => Err(TypeError::FreeVar),
        }
    }

    fn infer_symbol<'b, 'r>(&'b mut self, sym: &Symbol, given: NodeId) -> Result<'r, ()> {
        match self.get(&sym) {
            Some(t) => self.unify(t, given),
            None => Err(TypeError::FreeVar),
        }
    }

    fn infer_literal<'b, 'r>(&'b mut self, lit: &Literal, given: NodeId) -> Result<'r, ()> {
        use crate::prim::Literal::*;
        let ty = match lit {
            Int(_) => self.pool.ty_int(),
            Real(_) => self.pool.ty_real(),
            Char(_) => self.pool.ty_char(),
        };
        self.unify(given, ty)?;
        Ok(())
    }

    fn infer_constant<'b, 'r>(&'b mut self, _: &i64, given: NodeId) -> Result<'r, ()> {
        let ty = self.pool.ty_int();
        self.unify(given, ty)?;
        Ok(())
    }

    fn infer_char<'b, 'r>(&'b mut self, _: &u32, given: NodeId) -> Result<'r, ()> {
        let ty = self.pool.ty_char();
        self.unify(given, ty)?;
        Ok(())
    }

    fn infer_pat<'b, 'r>(&'b mut self, pat: &Pattern<NodeId>) -> Result<'r, ()> {
        use self::PatternKind::*;
        let ty = &pat.ty();
        match &pat.inner {
            Constant { value } => {
                self.infer_constant(value, *ty)?;
            }
            Char { value } => {
                self.infer_char(value, *ty)?;
            }
            Constructor { arg, name } => {
                let type_name = self
                    .symbol_table()
                    .get_datatype_of_constructor(name)
                    .expect("internal error: typing")
                    .clone();
                self.give(*ty, Typing::Datatype(type_name.clone()))?;
                if let Some(arg) = arg {
                    self.infer_pat(arg)?;
                    let arg_ty = self
                        .symbol_table()
                        .get_type(&type_name)
                        .expect("internal error: typing")
                        .constructors
                        .iter()
                        .find(|(cname, _)| cname == name)
                        .map(|(_, arg)| arg.clone())
                        .expect("internal error: typing")
                        .expect("internal error: typing");
                    let arg_typing = self.convert(arg_ty);
                    let arg_ty_id = self.pool.ty(arg_typing);
                    self.unify(arg.ty(), arg_ty_id)?;
                }
            }
            Tuple { tuple } => {
                for t in tuple {
                    self.infer_pat(t)?;
                }
                let tuple_ty = self
                    .pool
                    .ty(Typing::Tuple(tuple.iter().map(|pat| pat.ty()).collect()));
                self.unify(*ty, tuple_ty)?;
            }
            Wildcard { .. } | Variable { .. } => (),
        };
        for (name, ty) in pat.binds() {
            self.insert(name.clone(), *ty);
        }
        Ok(())
    }

    fn infer_tuple<'b, 'r>(
        &'b mut self,
        tuple: &Vec<CoreExpr<NodeId>>,
        given: NodeId,
    ) -> Result<'r, ()> {
        use std::iter;
        let tys = iter::repeat_with(|| self.pool.tyvar())
            .take(tuple.len())
            .collect::<Vec<_>>();

        for (e, t) in tuple.iter().zip(tys.iter()) {
            self.infer_expr(e)?;
            self.unify(e.ty(), *t)?;
        }
        let tuple_ty = self.pool.ty(Typing::Tuple(tys));
        self.unify(tuple_ty, given)?;
        Ok(())
    }

    fn unify<'b, 'r>(&'b mut self, id1: NodeId, id2: NodeId) -> Result<'r, ()> {
        self.pool.try_unify_with(id1, id2, try_unify).map(|_| ())
    }

    fn give<'b, 'r>(&'b mut self, id1: NodeId, ty: Typing) -> Result<'r, ()> {
        let id2 = self.pool.node_new(ty);
        self.unify(id1, id2)
    }
}

use crate::pass::Pass;
impl<'a> Pass<(SymbolTable, UntypedCore), TypeError<'a>> for Typer {
    type Target = (SymbolTable, TypedCore);

    fn trans<'b>(
        &'b mut self,
        (symbol_table, ast): (SymbolTable, UntypedCore),
        _: &Config,
    ) -> Result<'a, Self::Target> {
        let mut pass = self.generate_pass(symbol_table);
        let mut typing_ast = pass.pool.typing_ast(ast);
        pass.infer(&mut typing_ast)?;
        let typed_ast = pass.pool.typed_ast(typing_ast);

        let symbol_table = pass.into_symbol_table();
        Ok((symbol_table, typed_ast))
    }
}
