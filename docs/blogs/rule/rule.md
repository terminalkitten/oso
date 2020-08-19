Anatomy of a Rule
=================

Introduction
------------
Rules are the building blocks of any authorization policy, but the policies
that you can build with them are inherently constrained by their syntax,
semantics, and implementation. For example, an ordered list of `allow` or
`deny` statements with logical expressions over global variables like `ip`,
`request`, etc. is often sufficient for simple policies. But what happens
when your policy requires that the user's credentials be checked against
attributes of your application's data? How should you order rules that are
data-dependent? What if the rules need to index into a large permissions
matrix? What about delegation? Inheritance?

Authorization systems that handle these more complex cases sometimes do so by
[foisting the complexity onto the user](http://docs.oasis-open.org/xacml/3.0/errata01/os/xacml-3.0-core-spec-errata01-os-complete.html).
oso is an authorization system that was designed specifically to handle as
much of the complexity itself as possible, allowing you, the user, to
concentrate on the policy, not the vagaries of its expression or encoding.

A Simple Policy
---------------
Let's start simple. Suppose you're Marjory, proud manager of Alice and
Bhavik, who write reports on widgets for your mutual employer, BigCorp.
Let's say these reports are fairly sensitive (widgets being in high
demand), and so you need to protect against unauthorized report access.
The following informal statements might be a reasonable start to an
authorization policy:

* Alice is allowed to view and modify their own reports.
* Bhavik is allowed to view and modify their own reports.
* Marjory is allowed to view, but not modify, reports by both Alice and Bhavik.

In oso, you could express these statements in our domain-specific rule
language, Polar, like this:

```polar
allow("alice", action, resource) if
    action in ["GET", "PUT"] and
    resource.startswith("/reports/alice/");
allow("bhavik", action, resource) if
    action in ["GET", "PUT"] and
    resource.startswith("/reports/bhavik/");
allow("marjory", action, resource) if
    action in ["GET", "PUT"] and
    resource.startswith("/reports/");
```

(We'll leave out the authentication piece here; suppose that we've
already validated an OAuth JWT, say, to obtain and validate a user name
like `"alice"`.) If you put this policy in a file called `auth.polar`,
you can load it into oso from your Python application init code:

```python
from oso import Oso
oso = Oso()
oso.load_file("auth.polar")
```

And then when you get a request from `user` for some `resource`, you can call:

```python
oso.is_allowed(user, request.method, resource.path)
```

which will return `True` or `False` ("allowed" or "denied") based on the
supplied arguments by consulting the `allow` rules defined by your policy.

Code & Data
-----------
From the application's point of view, the arguments to
`is_allowed(actor, action, resource)` are *data*
(possibly from the application, or from an identity provider, etc.),
and the rules are *code* that is *evaluated* over that data to
make a decision. Speeding up query evaluation can thus be treated as
an exercise in interpreter (or compiler) design, and a standard set of
well-known tricks can be applied.

But another way to think of rules is as patterns, like regular expressions,
that can (fail to) *match* the supplied data, just as a regular expression
can (fail to) match a specific input string. A related view is as records
in a database; the arguments would then comprise the data values in the
query against those records. A database with three records is not hard to
make fast with trivial algorithms, but when we get into realistic sizes,
naive search techniques become impractically slow. Spending some time up
front (i.e., ahead of match/query time) to preprocess/index the
patterns/records can yield large savings at query time for certain kinds of
inputs/queries. And so a completely different, but also standard, set of
well-known tricks can be applied to make the search/query performant.

So which is it? Are rules code or data, fish or fowl? In the following
sections, we'll dive into the meaning and structure of rules in oso, and
see how and why oso is comfortable treating them as whatever your
authorization policy and dietary restrictions require.

Rules Abstractly
----------------
Rules drive the query process, and so the internals are interesting and
challenging from an implementation standpoint. But they are also the basic
unit of expression in our authorization language, and so are perhaps worth
talking about abstractly for a minute before we dive into the gory details
of the query execution model.

When you write a rule, you are expressing a piece of the authorization
logic for your application. If you were to express your policy in English,
rules would be the sentences: "Alice is allowed to view her own reports."
But English is not precise enough, and one of the basic premises of oso
is that most procedural and object oriented languages are poorly suited
to expressing complex sets of declarative logical statements. The syntax,
semantics, and implementation choices of rules all matter, because if any
of them obscure or inhibit the natural expression of your authorization
logic, then the system has not succeeded in its basic goal of simplifying
your implementation of a complex authorization policy.

Often the easiest way to express authorization policies is to have the
data that decisions are based on live in your application, and to use
application types (LINK) to call into your application when data is needed
to make a decision; this is (rougly) the "rules as code" point of view.
But other times it's easier or more convenient (especially when migrating
from another authorization system) to simply embed the necessary data
directly into the policy as literals (usually strings, but not necessarily).
This makes rules behave more like *data* than *code*, and the the rule
application process takes aspects of a (simple) database lookup. We've
built our rule engine so that such policies can still be fast, with no
additional work on your part.

From a design and implementation standpoint, then, rules must be powerful
enough to express all of the necessary authorization logic, concise enough
to not obscure that logic, and performant enough to render authorization
decisions in a timely manner.

Rules Concretely
----------------
So far we have talked about rules only abstractly. To be concrete, we'll
need to recall a few definitions from basic programming language theory.
Every function/procedure/rule/etc. has an associated list of *parameters*,
i.e., the expressions in parens in a function definition. In Python, `def
f(x, y): ...` has parameters `x` and `y`, which are both variables. Its
*body* is the stuff in `...`; it may refer to the variables `x` and `y`
with the assumption that they are bound. When you *call* a function, you
supply *arguments*; these are the values to which the parameter variables
are bound as part of the function call process. For instance, given the
prior definition, `f(1, 2)` calls the function named `f` with the arguments
`1` and `2`, which the variables `x` and `y`, respectively, are bound to in
the body of `f`.

Rules in Polar are roughly akin to functions in languages like Python,
or more precisely methods in a multiple dispatch object oriented language.
But as we'll see, there are also some important differences. Conceptually,
a `Rule` represents *one particular implementation* of a *predicate*,
which, when supplied at query time with a set of arguments that match its
parameters, is true just when a query for each conjunct in its body is also
true; otherwise it is false. There's the same kind of basic "match/bind
supplied arguments to parameters, then recursively do something with the
body" structure, but, crucially, the matching process for Polar rules
is significantly more expressive than simple binding.

All of this is still abstract. Concretely, rules are represented in Polar
by a very simple data structure:

```rust
pub struct Rule {
    pub name: Symbol,
    pub params: Vec<Parameter>,
    pub body: Term,
}
```

(This is [our actual definition](https://github.com/osohq/oso/blob/74f47b75d86f8386a97fedd251bfae5f1017558b/polar-core/src/types.rs#L506).)
A rule has a name (a `Symbol` is just a wrapper for a `String`), a vector
of parameters (we'll get to what's in `Parameter` shortly), and a body, which
we represent as a general `Term` (i.e., any Polar expression) for convenience,
but semantically it's always a (possibly empty) conjunction of terms.
An empty body corresponds to an empty conjunct, which is taken as true;
such rules need only match their parameters. For example, we can define
the rules:

```polar
odd(1);
even(2);
```
And then a query for the predicates `odd(1)` or `even(2)` will succeed
since they are true according to the rule definitions, but `odd(2)` or
`even(1)` will fail; [anything that is not known to be true is assumed
to be false](https://en.wikipedia.org/wiki/Closed-world_assumption).

All right, so what's in a `Parameter`? The example above shows that it
doesn't just have to be a variable, as in many languages. Indeed, it may
be any term:

```rust
pub struct Parameter {
    pub parameter: Term,
    pub specializer: Option<Term>,
}
```

The syntax in Polar is `parameter: specializer`. The formal `parameter`
is often, but not necessarily, a variable named by a symbol like `actor`,
but it may be an arbitrary term, and is matched with an equality relation
(unification). The `specializer` is a type restriction or declaration,
and is matched with a subtyping relation. The specializer is optional;
for example, the `actor` parameter in the rule below is unspecialized,
but the third parameter `_resource` is required to be a `Report` whose
`author` attribute is whatever the value of `actor` is:

```polar
allow(actor, "GET", _resource: Report{author: actor});
```

(The leading `_` on `_resource` is just to tell Polar that we know that
variable isn't used anywhere, just matched via the specializer. If we
don't, it will issue a "singleton variable" warning when the rule is
loaded, because that sometimes indicates a logic bug.) Given the rule above
and an appropriate `Report` class definition, a query for the predicate:

```polar
allow("alice", "GET", new Report{author: "alice"})
```

should succeed (CHECK), and so, therefore, should the Python call:

```python
is_allowed("alice", "GET", Report(author="alice"))
```

Rule Lookup
-----------
Now let's follow the query logic and see how it interacts with the rules.
We start with:

```polar
allow("alice", "GET", new Report{author: "alice"})
```

The name of the predicate being queried is `allow`, so the first thing to
do is get all the `allow` rules. We keep these in a hash table indexed by
name, so this lookup is cheap. What we get back isn't just a vector of
`Rule` instances, though; it's what we call a `GenericRule`:

```rust
pub struct GenericRule {
    pub name: Symbol,
    next_rule_id: u64,
    rules: HashMap<u64, Arc<Rule>>,
    index: RuleIndex,
}
```

This is the data structure we'll discuss for most of the rest of this article.

Generic Rules
-------------
First, the name. A `GenericRule` is not "generic" in the sense of being
"unspecific", nor is it used in the sense of "generics", i.e.,
"generic over a range of parameterized types" like `<T>` in Rust, Java,
C++, etc. It is rather meant in the sense of
[generic functions](http://clhs.lisp.se/Body/26_glo_g.htm#generic_function)
in Common Lisp or [Julia](https://docs.julialang.org/en/v1/manual/methods/).
Individual rules (methods) implement a piece of the overall "generic rule";
the "piece" is the set of arguments its parameters match. The behavior
of the generic rule as a whole is completely determined by the rules
that comprise it, together with a strategy for matching them against
a supplied set of arguments and ordering the results of that match.

For example, suppose we represent users as instances of a `User` class,
and that there's a privileged `SuperUser` subclass:

```python
class User:
    ...
class SuperUser(User):
    ...
```

These classes may be used as specializers, so we might have a pair of rules
like this:

```polar
allow(user: User, action, resource);
allow(user: SuperUser, action, resource);
```

Rule Sorting
------------
Now, in which order shall we consult these two rules? The first is
obviously defined first, so that might be the natural choice. Another
natural choice might be to have it simply not matter, and to insist
that rule application be a commutative operation. But this strategy
does not work if you want to be able to *override* behavior specialized
on *less specific* classes (i.e., superclasses). This ability is important
in several kinds of authorization scenarios; e.g., if a default logging or
warning method should be overridden for a super user, `deny`, etc.

We therefore *sort* rules according to the specificity of their specializers
in left-to-right order. In the rules above, the first parameter's
specializer is more specific in the second rule with respect to
any instance of `User` or a subclass thereof, and so it is selected first.

The particulars of the rule sorting algorithm are somewhat complex,
and may be covered in a future article. In short, we can't just call
`rules.sort()` on some vector of rules, because the comparison predicate
may need to perform FFI calls into the application to determine whether
one application class is more specific than another with respect to some
argument. But details aside, the result of the process is a list of rules
in most-to-least specific order with respect to the given arguments.

Rule Filtering
--------------
We could sort every rule defined as part of a generic rule, but doing
so would be rather slow, especially with a large number of rules. So we
*filter* the rules by applicability prior to sorting them. That means
we check each rule *prior* to sorting, and reject it if the arguments
don't match the parameters. For example, if an argument isn't of the
type required by the corresponding parameter specializer, the rule is
not applicable, and so we should not bother to sort or execute it.

Rules as Data
-------------
The model of generic rules we've described so far works well for the
"rules as code" point of view: rules are like (multi) methods that
collectively comprise the behavior of a predicate. But what if we
want to take another (completely valid) point of view: that the rules
represent *data* about the allowable combinations of actor, action,
and resource, and it is to be *matched* against a particular triple
of values.

For example, suppose we already had a complex authorization system
that used a permissions matrix of the form: ...

Now, one simple way to get this data into oso is to mechanically translate
it into rules of the form: ...

If the matrix is large, there will be *many* such rules. But the rules
aren't specialized in the same sort of way as the ones above; they're
less "code" and more "data".

Similar situations may arise importing data from an identity provider such
as LDAP, or if you simply have a lot of users and want to represent them
directly in oso.

Pre-filtering
-------------
We can't tell the difference syntactically between a "code" rule and a
"data" rule, because there is in fact no local distinction; it's a global
property of the policy, not a local property of the rules themselves. So
we can't give up semantics that are important for the "code" view, but it
turns out that enforcing those semantics for "data" style rules is *slow*
if you don't compensate for the (much) greater number of rules.

Our solution is to *pre-filter* rules based on an index that's computed
ahead of time (as rules are added to the generic function). Pre-filtering
quickly prunes rules that are not applicable to the supplied arguments.

Consider this set of rules: ... rules w/lots of literals ...
We build an index like so ...

Indices are automatic; you don't need to "opt in".

Indices are only over ground terms, but can "look over" variables or
application instances.

We also pre-filter lists, so that lists like `["GET", "POST"]` above,
and especially much longer examples, are also fast.

Generic Rules Redux
-------------------
We've now seen how individual rule definitions (i.e., the statements
you write in your policy) are grouped into generic rules. The generic
rules represent predicates that are queried with a given set of arguments.
That query process selects rules that are applicable to the arguments,
sorts them in most-to-least-specific order, and applies them one at a
time to the arguments. Cases where there are many rules with constant
parameters (i.e., "data") are sped up using precomputed indexes over
those values.

This all sounds quite complex, and in a sense it is. But our reasons for
doing it in oso are simple: so that you don't have to. *Any* rule system
must decide how to order the rules that have been loaded; simple rule
systems rely on simple heuristics such as the order of definition, but for
more complex policies those become untenable. Some systems allow rules to
be ordered in various ways, but make *you* specify *how*. We think that
such concerns are best handled automatically, in an intuitive and flexible
way, in particular by leveraging subtyping relations (inheritance) that
you've already defined for your application classes.

Conclusion
----------
...
