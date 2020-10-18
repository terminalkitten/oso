use super::partial::Constraints;
use super::rules::*;
use super::terms::*;

/// Cases to cover:
///
/// 1. Renaming variables: &Symbol -> Option<Symbol>
/// 2. Rewriting terms: &Operation -> Option<Term>
/// 3. Simplifier: ???

pub trait Visitor: Sized {
    // Atoms. These may be overridden as needed.
    fn visit_number(&mut self, _n: &Numeric) -> Option<Numeric> {
        None
    }
    fn visit_string(&mut self, _s: &str) -> Option<String> {
        None
    }
    fn visit_boolean(&mut self, _b: &bool) -> Option<bool> {
        None
    }
    fn visit_id(&mut self, _i: &u64) -> Option<u64> {
        None
    }
    fn visit_name(&mut self, _n: &Symbol) -> Option<Symbol> {
        None
    }
    fn visit_variable(&mut self, _v: &Symbol) -> Option<Symbol> {
        None
    }
    fn visit_rest_variable(&mut self, _r: &Symbol) -> Option<Symbol> {
        None
    }
    fn visit_operator(&mut self, _o: &Operator) -> Option<Operator> {
        None
    }

    // Compounds. If you override these, you must walk the children manually.
    fn visit_rule(&mut self, r: &Rule) -> Option<Rule> {
        walk_rule(self, r)
    }
    fn visit_term(&mut self, t: &Term) -> Option<Term> {
        walk_term(self, t)
    }
    fn visit_field(&mut self, k: &Symbol, v: &Term) -> Option<(Symbol, Term)> {
        walk_field(self, k, v)
    }
    fn visit_external_instance(&mut self, e: &ExternalInstance) -> Option<ExternalInstance> {
        walk_external_instance(self, e)
    }
    fn visit_instance_literal(&mut self, i: &InstanceLiteral) -> Option<InstanceLiteral> {
        walk_instance_literal(self, i)
    }
    fn visit_dictionary(&mut self, d: &Dictionary) -> Option<Dictionary> {
        walk_dictionary(self, d)
    }
    fn visit_pattern(&mut self, p: &Pattern) -> Option<Pattern> {
        walk_pattern(self, p)
    }
    fn visit_call(&mut self, c: &Call) -> Option<Call> {
        walk_call(self, c)
    }
    #[allow(clippy::ptr_arg)]
    fn visit_list(&mut self, l: &TermList) -> Option<TermList> {
        walk_list(self, l)
    }
    fn visit_operation(&mut self, o: &Operation) -> Option<Operation> {
        walk_operation(self, o)
    }
    fn visit_param(&mut self, p: &Parameter) -> Option<Parameter> {
        walk_param(self, p)
    }

    #[allow(clippy::ptr_arg)]
    fn visit_params(&mut self, p: &Vec<Parameter>) -> Option<Vec<Parameter>> {
        walk_params(self, p)
    }

    fn visit_partial(&mut self, c: &Constraints) -> Option<Constraints> {
        walk_partial(self, c)
    }
}

macro_rules! walk_elements {
    ($visitor: expr, $method: ident, $list: expr) => {
        // for el in $list {
        //     $visitor.$method(el);
        // }
        // find the first changed term
        // if exists, clone the first i-1 terms, chain the new term
        // and fold the remaining terms
        // otherwise, no changes needed
        if let Some((idx, last)) = $list
            .iter()
            .enumerate()
            .find_map(|(idx, el)| $visitor.$method(el).map(|v| (idx, v)))
        {
            Some(
                $list
                    .iter()
                    .take(idx)
                    .cloned()
                    .chain(Some(last))
                    .chain(
                        $list
                            .iter()
                            .skip(idx + 1)
                            .map(|el| $visitor.$method(el).unwrap_or_else(|| el.clone())),
                    )
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        }
    };
}

macro_rules! walk_fields {
    ($visitor: expr, $method: ident, $dict: expr) => {
        // for (k, v) in $dict {
        //     $visitor.$method(k, v);
        // }
        // find the first changed term
        // if exists, clone the first i-1 terms, chain the new term
        // and fold the remaining terms
        // otherwise, no changes needed
        if let Some((idx, last)) = $dict
            .iter()
            .enumerate()
            .find_map(|(idx, (k, v))| $visitor.$method(k, v).map(|v| (idx, v)))
        {
            Some(
                $dict
                    .iter()
                    .take(idx)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .chain(Some(last))
                    .chain($dict.iter().skip(idx + 1).map(|(k, v)| {
                        $visitor
                            .$method(k, v)
                            .unwrap_or_else(|| (k.clone(), v.clone()))
                    }))
                    .collect::<std::collections::BTreeMap<_, _>>(),
            )
        } else {
            None
        }
    };
}

/// Unwrap the optionally set field $param, using the same-named field
/// from the original value $orig otherwise
///
/// If the field itself is an optional, do the same mechanism but on the
/// nested fields
macro_rules! unwrap_or_default {
    ($param:ident, $orig:expr) => {
        $param.unwrap_or_else(|| $orig.$param.clone())
    };
    (@opt $param:ident, $orig:expr) => {
        // this unwrap here is okay because $param is only Some if $orig.$param is Some
        $param.map(|$param| $param.unwrap_or_else(|| $orig.$param.clone().unwrap()))
    };
}

pub fn walk_rule<V: Visitor>(visitor: &mut V, rule: &Rule) -> Option<Rule> {
    match (
        visitor.visit_name(&rule.name),
        walk_elements!(visitor, visit_param, &rule.params),
        visitor.visit_term(&rule.body),
    ) {
        (None, None, None) => None,
        (name, params, body) => Some(Rule {
            name: name.unwrap_or_else(|| rule.name.clone()),
            params: params.unwrap_or_else(|| rule.params.clone()),
            body: body.unwrap_or_else(|| rule.body.clone()),
        }),
    }
}

pub fn walk_term<V: Visitor>(visitor: &mut V, term: &Term) -> Option<Term> {
    match term.value() {
        Value::Number(n) => visitor.visit_number(n).map(Value::Number),
        Value::String(s) => visitor.visit_string(s).map(Value::String),
        Value::Boolean(b) => visitor.visit_boolean(b).map(Value::Boolean),
        Value::ExternalInstance(e) => visitor
            .visit_external_instance(e)
            .map(Value::ExternalInstance),
        Value::InstanceLiteral(i) => visitor
            .visit_instance_literal(i)
            .map(Value::InstanceLiteral),
        Value::Dictionary(d) => visitor.visit_dictionary(d).map(Value::Dictionary),
        Value::Pattern(p) => visitor.visit_pattern(p).map(Value::Pattern),
        Value::Call(c) => visitor.visit_call(c).map(Value::Call),
        Value::List(l) => visitor.visit_list(l).map(Value::List),
        Value::Variable(v) => visitor.visit_variable(v).map(Value::Variable),
        Value::RestVariable(r) => visitor.visit_rest_variable(r).map(Value::RestVariable),
        Value::Expression(o) => visitor.visit_operation(o).map(Value::Expression),
        Value::Partial(p) => visitor.visit_partial(p).map(Value::Partial),
    }
    .map(|v| term.clone_with_value(v))
}

pub fn walk_field<V: Visitor>(
    visitor: &mut V,
    key: &Symbol,
    value: &Term,
) -> Option<(Symbol, Term)> {
    match (visitor.visit_name(key), visitor.visit_term(value)) {
        (None, None) => None,
        (k, v) => Some((
            k.unwrap_or_else(|| key.clone()),
            v.unwrap_or_else(|| value.clone()),
        )),
    }
}

pub fn walk_external_instance<V: Visitor>(
    visitor: &mut V,
    instance: &ExternalInstance,
) -> Option<ExternalInstance> {
    visitor.visit_id(&instance.instance_id);
    // TODO: doesn't make sense to combine here, right?
    None
}

pub fn walk_instance_literal<V: Visitor>(
    visitor: &mut V,
    instance: &InstanceLiteral,
) -> Option<InstanceLiteral> {
    match (
        visitor.visit_name(&instance.tag),
        walk_fields!(visitor, visit_field, &instance.fields.fields),
    ) {
        (None, None) => None,
        (tag, fields) => Some(InstanceLiteral {
            tag: unwrap_or_default!(tag, instance),
            fields: Dictionary {
                fields: unwrap_or_default!(fields, instance.fields),
            },
        }),
    }
}

pub fn walk_dictionary<V: Visitor>(visitor: &mut V, dict: &Dictionary) -> Option<Dictionary> {
    walk_fields!(visitor, visit_field, &dict.fields).map(|fields| Dictionary { fields })
}

pub fn walk_pattern<V: Visitor>(visitor: &mut V, pattern: &Pattern) -> Option<Pattern> {
    match pattern {
        Pattern::Dictionary(dict) => visitor.visit_dictionary(dict).map(Pattern::Dictionary),
        Pattern::Instance(instance) => visitor
            .visit_instance_literal(&instance)
            .map(Pattern::Instance),
    }
}

pub fn walk_call<V: Visitor>(visitor: &mut V, call: &Call) -> Option<Call> {
    match (
        visitor.visit_name(&call.name),
        walk_elements!(visitor, visit_term, &call.args),
        call.kwargs
            .as_ref()
            .map(|kwargs| walk_fields!(visitor, visit_field, &kwargs)),
    ) {
        (None, None, None) => None,
        (name, args, kwargs) => Some(Call {
            name: unwrap_or_default!(name, call),
            args: unwrap_or_default!(args, call),
            kwargs: unwrap_or_default!(@opt kwargs, call),
        }),
    }
}

#[allow(clippy::ptr_arg)]
pub fn walk_list<V: Visitor>(visitor: &mut V, list: &TermList) -> Option<TermList> {
    walk_elements!(visitor, visit_term, list)
}

pub fn walk_operation<V: Visitor>(visitor: &mut V, expr: &Operation) -> Option<Operation> {
    match (
        visitor.visit_operator(&expr.operator),
        walk_elements!(visitor, visit_term, &expr.args),
    ) {
        (None, None) => None,
        (operator, args) => Some(Operation {
            operator: unwrap_or_default!(operator, expr),
            args: unwrap_or_default!(args, expr),
        }),
    }
}

pub fn walk_param<V: Visitor>(visitor: &mut V, param: &Parameter) -> Option<Parameter> {
    match (
        visitor.visit_term(&param.parameter),
        param
            .specializer
            .as_ref()
            .map(|spec| visitor.visit_term(spec)),
    ) {
        (None, None) => None,
        (parameter, specializer) => Some(Parameter {
            parameter: unwrap_or_default!(parameter, param),
            specializer: unwrap_or_default!(@opt specializer, param),
        }),
    }
}

#[allow(clippy::ptr_arg)]
pub fn walk_params<V: Visitor>(visitor: &mut V, params: &Vec<Parameter>) -> Option<Vec<Parameter>> {
    walk_elements!(visitor, visit_param, &params)
}

pub fn walk_partial<V: Visitor>(visitor: &mut V, partial: &Constraints) -> Option<Constraints> {
    match (
        visitor.visit_name(&partial.variable),
        walk_elements!(visitor, visit_operation, &partial.operations),
    ) {
        (None, None) => None,
        (variable, operations) => Some(Constraints {
            variable: unwrap_or_default!(variable, partial),
            operations: unwrap_or_default!(operations, partial),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestVisitor {
        visited: Vec<Value>,
    }

    impl TestVisitor {
        fn new() -> Self {
            Self { visited: vec![] }
        }
        fn push(&mut self, value: Value) {
            self.visited.push(value);
        }
    }

    impl Visitor for TestVisitor {
        fn visit_number(&mut self, n: &Numeric) -> Option<Numeric> {
            self.push(Value::Number(*n));
            None
        }
        fn visit_string(&mut self, s: &str) -> Option<String> {
            self.push(Value::String(s.to_string()));
            None
        }
        fn visit_boolean(&mut self, b: &bool) -> Option<bool> {
            self.push(Value::Boolean(*b));
            None
        }
        fn visit_id(&mut self, i: &u64) -> Option<u64> {
            self.push(Value::Number(Numeric::Integer(*i as i64)));
            None
        }
        fn visit_name(&mut self, n: &Symbol) -> Option<Symbol> {
            self.push(Value::Variable(n.clone()));
            None
        }
        fn visit_variable(&mut self, v: &Symbol) -> Option<Symbol> {
            self.push(Value::Variable(v.clone()));
            None
        }
        fn visit_rest_variable(&mut self, r: &Symbol) -> Option<Symbol> {
            self.push(Value::RestVariable(r.clone()));
            None
        }
        fn visit_operator(&mut self, o: &Operator) -> Option<Operator> {
            self.push(Value::Expression(Operation {
                operator: *o,
                args: vec![],
            }));
            None
        }
    }

    #[test]
    fn test_walk_term_atomics() {
        let number = value!(1);
        let string = value!("Hi there!");
        let boolean = value!(true);
        let variable = value!(sym!("x"));
        let rest_var = Value::RestVariable(sym!("rest"));
        let list = Value::List(vec![
            term!(number.clone()),
            term!(string.clone()),
            term!(boolean.clone()),
            term!(variable.clone()),
            term!(rest_var.clone()),
        ]);
        let term = term!(list);
        let mut v = TestVisitor::new();
        v.visit_term(&term);
        assert_eq!(v.visited, vec![number, string, boolean, variable, rest_var]);
    }

    #[test]
    fn test_walk_term_compounds() {
        let external_instance = term!(Value::ExternalInstance(ExternalInstance {
            instance_id: 1,
            constructor: None,
            repr: None,
        }));
        let instance_pattern = term!(value!(Pattern::Instance(InstanceLiteral {
            tag: sym!("d"),
            fields: Dictionary {
                fields: btreemap! {
                    sym!("e") => term!(call!("f", [2])),
                    sym!("g") => term!(op!(Add, term!(3), term!(4))),
                }
            }
        })));
        let dict_pattern = term!(Value::Pattern(Pattern::Dictionary(Dictionary {
            fields: btreemap! {
                sym!("i") => term!("j"),
                sym!("k") => term!("l"),
            },
        })));
        let term = term!(btreemap! {
            sym!("a") => term!(btreemap!{
                sym!("b") => external_instance,
                sym!("c") => instance_pattern,
            }),
            sym!("h") => dict_pattern,
        });
        let mut v = TestVisitor::new();
        v.visit_term(&term);
        assert_eq!(
            v.visited,
            vec![
                value!(sym!("a")),
                value!(sym!("b")),
                value!(1),
                value!(sym!("c")),
                value!(sym!("d")),
                value!(sym!("e")),
                value!(sym!("f")),
                value!(2),
                value!(sym!("g")),
                value!(op!(Add)),
                value!(3),
                value!(4),
                value!(sym!("h")),
                value!(sym!("i")),
                value!("j"),
                value!(sym!("k")),
                value!("l"),
            ]
        );
    }

    #[test]
    fn test_walk_rule() {
        let rule = rule!("a", ["b"; instance!("c"), value!("d")] => call!("e", [value!("f")]));
        let mut v = TestVisitor::new();
        v.visit_rule(&rule);
        assert_eq!(
            v.visited,
            vec![
                value!(sym!("a")),
                value!(sym!("b")),
                value!(sym!("c")),
                value!("d"),
                value!(op!(And)),
                value!(sym!("e")),
                value!("f"),
            ]
        );
    }

    // TODO(gj): Add test for walking a partial.
}
