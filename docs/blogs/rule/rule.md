Anatomy of a Rule
=================

Introduction
------------
Rules are the building blocks of any authorization policy.
But they are often used for two different purposes:

* To express abstract authorization logic
* To encode concrete permissions data

An access control list (ACL) is an example of the latter: you write
a (long) list of who can do what. But role-based access control (RBAC)
involves logical operations: *if* user `U` is an element of group `G`
and group `G` has permission `Y`, *then* `U` has permission `Y`.

An authorization system that goes beyond ACLs and RBAC needs to
handle rules of both sorts. Sometimes you want the flexiblity of a full
programming language — certainly at least powerful logical expressions,
and for complex policies, types and inheritance. But other times you might
need to encode a whole permissions matrix, and would like lookups to be
fast even over a large matrix. These are more aspects of a data store
than a programming language.

In this article, we'll take a deep dive into the anatomy of a rule in oso,
and how & why we built the interpreter to support both models. Spoiler:
it's a Prolog interpreter with some fancy indexing and a swanky FFI.
We've added some tricks — in accordance with our
[third principle](https://docs.oso.dev/more/design-principles.html),
we freely extended the basic model to make it suit our applications.
We're
[not](https://opensource.google/projects/cel)
[the](https://www.openpolicyagent.org/docs/latest/policy-reference/)
[only](https://github.com/rust-lang/chalk)
[ones](https://stedolan.github.io/jq/)
[doing](https://github.com/shop-planner/shop3)
[this](https://allegrograph.com/products/allegrograph/).
We think that's a strength: we're building on a common framework and
execution model that's proved its strengths and flexibility over many
decades.

oso
---
In order to discuss concrete design decisions and implementation choices,
we'll need to briefly describe the basic architecture of oso. *oso* is an
authorization system designed to secure *applications*. In this article
we'll assume the application is written in Python, but Ruby, Java, and
Node.js are also supported, with more language integrations coming soon.

To add authorization to an application using oso, you can:

1. Import the oso library: `from oso import Oso`
2. Make an `Oso` instance: `oso = Oso()`
3. Load a policy file: `oso.load_file("policy.polar")`
4. Call the authorization method: `oso.is_allowed(actor, action, resource)`

(This is the completely manual version. If you're using a web framework
integration, it may be somewhat simpler.) The first three steps are setup,
and should normally be done once in application initialization. The last is
the authorization call: if it returns `True`, then the supplied `actor` is
authorized to perform `action` on `resource` according to the policy loaded
in step 3. All three arguments may be arbitrary objects.

The setup above omits a crucial step, of course:

0. Write the authorization policy: `"policy.polar"`

Policies in oso are written in a domain specific language called *Polar*.
This article is mostly about the structure of the primary unit of
composition in Polar, the *rule*. Its goal is to show some of the
motivation behind the design of their syntax and semantics, and how those
might affect how you think about and write your authorization policy.

Along the way, we'll get into some details of our interpreter for the
Polar langauge, which is a based on [a small virtual machine written
in Rust](https://github.com/osohq/oso/blob/main/polar-core/src/vm.rs).
This Rust core is shared by all the application language integrations,
which communicate with the Rust code via a simple event-based interface
over FFI. This architecture allows for extremely efficient bidirectional
communication between the application and authorization system, which,
as we'll see, in turn allows a great deal of flexibility with regard
to the location and type of data used to make authorization decisions.

Rules in oso
------------
To begin our discussion of rules, let's take a simple example policy.
The only "distinguished" rule in oso is named `allow`, and it is
distinguished only by convention; there's nothing special about it.
Simple definitions might look like this:

```polar
allow("alice", "GET", "/reports/alice/");
allow("bhavik", "GET", "/reports/bhavik/");
allow("marjory", "GET", "/reports/alice/");
allow("marjory", "GET", "/reports/bhavik/");
allow("marjory", "PUT", "/reports/alice/");
allow("marjory", "PUT", "/reports/bhavik/");
```

Here we've got a handful of `allow` rules for the *actors* Alice, Bhavik,
and Marjory, *actions* `"GET"` and `"PUT"`, and *resources* that are paths
to (presumably) reports. We can plausibly infer from this policy that Marjory
is a manager of some sort for Alice and Bhavik, but we have chosen not to
encode that information explicitly yet.

Suppose our web application receives a `"GET"` request for `"/reports/alice/"`
from `"marjory"` — authenticated, say, via OAuth, etc. The application wishes
to know if this request is permitted according to the policy above, so it calls:

```python
oso.is_allowed("marjorie", "GET", "/reports/alice/")
```

The `oso.is_allowed(...)` call performs the authorization check
(and so must be fast enough to do on every request). It returns
`True` or `False` depending on whether the supplied actor, action,
and resource arguments successfully match the `allow` rules defined.
It does so by issuing a *query* to the policy engine:
`allow("marjory", "GET", "/reports/alice/")`. In this case, there
is a direct match with the third rule. But there would not be,
for instance, if the actor were `"bhavik"` instead of `"marjorie"`
or the request were `"DELETE"` instead of `"GET"`, since we have
not defined rules that would match those arguments.

Code & data
-----------
From the application's point of view, the arguments to
`is_allowed(actor, action, resource)` are *data*
(from the application, the request, an identity provider, etc.),
and the rules are *code* that is *evaluated* over that data to
make a decision. Making query evaluation fast can thus be treated as
an exercise in interpreter (or compiler) design, and a standard set
of well-known tricks can be applied.

But another way to think of rules is as patterns, like regular expressions,
that can (fail to) *match* the supplied data, just as a regular expression
can (fail to) match a specific input string. A related view is as records
in a database; the arguments would then comprise the data values in the
query against those records. A database with six records is not hard to
make fast with trivial algorithms, but when we get into realistic sizes,
naive search techniques become impractically slow. Spending some time
up front (i.e., ahead of match/query time) to preprocess/index the
patterns/records can yield large savings at query time for certain kinds
of inputs/queries. And so a completely different set of standard tricks
can be applied to make the search/query fast.

So which is it? Are rules code or data, fish or fowl? In the following
sections, we'll explore the meaning and structure of rules in oso,
and see how and why oso is comfortable treating them as whatever your
authorization policy and dietary restrictions require.

What's in a rule?
-----------------
Let's examine the structure of a rule now, and see what makes it tick.
Polar rules are roughly akin to functions in languages like Python,
or more precisely methods in a multiple dispatch object oriented language.
Abstractly, rules are piece-wise definitions of a *predicate*, a logical
proposition that is either *true* or *false* when we query for it.
That is, all of the rule definitions we've seen so far are "pieces"
of the predicate `allow(actor, action, resource)`. Each rule is
*applicable* only to certain queries for that predicate; namely, queries
whose supplied arguments *match* the *parameters* defined for that rule.
We'll talk more about the matching process below, but in short, it's a
combination of equality & binding (unification), structure matching over
compound types, and class-based type restrictions (instance-of).

OK, enough abstraction. The concrete syntax of rule definitions in
Polar is the rule name, followed by the parameter list, optionally
followed by the keyword `if` and a body (Prolog aficionados will
recognize this as a phonetic spelling of `:-`). So a rule with no
body is defined like this:

```polar
allow(actor, action, resource);
```

This rule will match *any* arguments by simply binding the parameter
variables to them. If we want the rule to match conditionally, we can
either restrict the parameters, as here to exact values:

```polar
allow("alice", "GET", "/reports/alice/");
```

Or we can add a body:

```polar
allow(actor, action, resource) if
    actor = "alice" and
    action = "GET" and
    resource = "/reports/alice/";
```

These two definitions mean exactly the same thing.

We've now see the three attributes — name, parameter list, and body —
that are all we need to represent rules in the Polar virtual machine:

```rust
pub struct Rule {
    pub name: Symbol,
    pub params: Vec<Parameter>,
    pub body: Term,
}
```

(This is [our actual definition](https://github.com/osohq/oso/blob/74f47b75d86f8386a97fedd251bfae5f1017558b/polar-core/src/types.rs#L506).)
As we've seen, a `Parameter` need not just be a variable, as in most
languages; e.g., you can't write `def allow("alice", ...)` in Python.
But in Polar it may be any term:

```rust
pub struct Parameter {
    pub parameter: Term,
    pub specializer: Option<Term>,
}
```

There's also an optional *specializer*, which is a type restriction
that the parameter must satisfy. The syntax in Polar is `parameter: specializer`,
where the specializer may be any recognized type, possibly including
type restrictions on its attributes. In particular, it may be an
[application type](https://docs.oso.dev/getting-started/policies/application-types.html),
and will respect the subtyping semantics of the application language.

For example, we might choose to represent reports not by their paths,
but by instances of a (registered) application class. We can decorate
(or manually register) our Python class:

```python
@polar_class
class Report:
   ...
```

And in the Polar policy, we can now use `Report` as a type specializer:

```polar
allow(actor, "GET", resource: Report{author: actor});
```

This is read: "allow `actor` to `GET` a `Report` that they wrote".
Then the query:

```polar
allow("alice", "GET", new Report{author: "alice"})
```

should succeed, and so, therefore, should the Python call:

```python
is_allowed("alice", "GET", Report(author="alice"))
```

The type restriction `Report` also matches instances of any subclass,
because specializers are matched via subtyping. E.g., if we have:

```python
@polar_class
class SpecialReport(Report):
    ...
```

And then call:

```python
is_allowed("alice", "GET", SpecialReport(author="alice"))
```

This result will also be true. We can see here how Python instances are
represented within Polar, and how it is possible to construct them from
either language and pass them seamlessly back and forth. You can do this
with any supported host language (Python, Ruby, Java, and Node.js so far).

Selecting & filtering rules
---------------------------
Suppose that instead of just Alice, Bhavik, and Marjory, we have a whole
organization's worth of people to authorize. Especially if we're migrating
from another authorization system, the easiest way to get authorization up
and running quickly might be to mechanically (and possibly automatically)
convert a permissions matrix into a set of Polar rules:

```polar
allow("alice", "GET", "/reports/alice/");
allow("alice", "PUT", "/reports/alice/");
allow("bhavik", "GET", "/reports/bhavik/");
allow("bhavik", "PUT", "/reports/bhavik/");
allow("charlie", "GET", "/reports/charlie/");
allow("charlie", "PUT", "/reports/charlie/");
...
allow("zed", "GET", "/reports/zed/");
allow("zed", "PUT", "/reports/zed/");
```

This policy does not exploit any of the structure inherent in the data; it
just encodes it directly. The problem, of course, is that such policies
quickly grow enormous. Abstracting policy types and logic can help, but
often there is still a core data set that is most naturally expressed
directly.

Let's see what happens for a query like:

```polar
allow("zed", "GET", "/reports/alice/")
```

This query should fail (i.e., be false), because Zed is not allowed
to view Alice's reports. But in a naive implementation, we would need
to try matching *every `allow` rule defined* to make this determination.
For large rule sets, the performance of this strategy quickly becomes
unacceptable.

But notice that none of the parameters of the rules above contain
variables or specializers; they are *ground* terms, whose values are
*constant*. Moreover, all of the arguments to the query predicate
are also ground; we're not asking for a *set* of authorized actors
for this action on this resource, we're asking is *this* user authorized
to take *this* action on *this* resource; all of those values are ground,
too. This observation enables us to build, ahead of time, an index over
the ground parameters of rules that lets us do very fast parameter/argument
matching in the (common) case where the arguments are also ground. We use
a sparse trie of hash tables in our implementation (the standard choice),
and have found that it can speed up realistic queries over large,
data-intensive policies by an order of magnitude or more. Specifically,
the speedup comes from removing rules from consideration that can quickly
be determined to be inapplicable. We call this our *rule pre-filter*.
Its goal is get the size of the set of rules that must be considered
in detail down from an arbitrarily large number to, ideally, one or
a few.

The rules that remain after pre-filtering *could* be applicable to the
supplied query arguments; the index will rule out ones that *can't* be
applicable, but it can't decide for non-ground terms. The job of the rule
*filter* is to eliminate those that are *actually* not applicable in the
current dynamic context. It does this by attempting to unify the argument
with the parameter and match it against the specializer (if there is one).
Either of those could potentially require an FFI round-trip to the
application to answer, which is much more expensive than an index lookup.

Sorting rules
-------------
Having filtered our rules for applicability, we know that the remaining
rules could succeed (but may not if they have bodies). But in what order
should we query them? The order of definition is one natural order, and
in fact our whole rule selection & sorting process is stable with respect
to that ordering. But we also impose a stronger, and, we think, more useful
ordering: *more specific rules run first*. In data-heavy policies, the
ordering usually doesn't matter; but if your policy is organized around
domain model classes with inheritance, then being able to *override* rules
defined on less specific (i.e., more general) superclasses can be extremely
valuable.

We therefore *sort* the applicable rules by *specificity*. The details
of this relation are somewhat involved (see, e.g.,
[JLS §15.12.2.5](https://docs.oracle.com/javase/specs/jls/se8/html/jls-15.html#jls-15.12.2.5),
[Common Lisp §7.6.6.1.2](http://clhs.lisp.se/Body/07_ffab.htm)),
but the basics are simple: two terms that unify are identically specific;
a subclass is more specific than any of its superclasses; and specializers
with attributes (fields) are more specific than those without. The
difficulty is that we can't, in general, make some of those determinations
without asking the application. (E.g., think about what happens if your
classes have a custom equality method, which changes the semantics of
unification; or use metaclasses to change the semantics of inheritance,
etc.) And so we can't just call `rules.sort()` on a vector of rules,
because the comparison function itself could require an FFI round-trip.

This design constraint made implementing the sort itself an interesting
challenge. Our solution was to hand compile a simple sorting algorithm
with explicit state management directly into VM instructions, so that it
can use FFI calls to answer questions like "is class `X` more specific
than class `Y` with respect to object `z`?" essentially as subroutines
of its comparison predicate.

For a few rules, this process is reasonably fast, but for many rules
it can be a performance bottleneck, even with caches; hence the importance
of the filtering stages described above. If the filters can get the set
of applicable rules down to a singleton, then the sort is obviously trivial,
and takes no time.

Generic rules
-------------
The process we've just described for selecting, filtering, and sorting
applicable rules is part of what we call our *generic rule* implementation.
We don't mean "generic" in the sense of "unspecific", nor do we mean
"generic over a range of parameterized types" like `<T>`
in Rust, Java, C++, etc. It is meant rather in the sense
of [generic functions in Common Lisp](http://clhs.lisp.se/Body/07_f.htm)
or [Clojure](https://clojure.org/reference/multimethods)
or [Julia](https://docs.julialang.org/en/v1/manual/methods/):
individual rules (methods) implement a "slice" of the overall behavior,
defined over the set of arguments its parameters match. The behavior of the
generic rule as a whole is completely determined by the rules that comprise
it, together with a specified strategy for matching them against a supplied
set of arguments and ordering the results of that match.

Some languages (e.g., Python or Java), support methods specialized
only over their first argument (`self` or `this`). But generic rules
may be specialized on *any* argument, or all of them; they are
*multi-methods*. We think this is an important property for authorization
rules to have, because so much of authorization is dependent on the
*relations between* the actor, action and resource, not just properties
or methods on one — the question is always *which* one, and with respect
to which other? With multi-methods, you are not forced to artificially
choose; you may simply express the relationship directly.

Let's make this concrete. We showed above some `allow` rules that
specialized their `resource` arguments on (subclasses of) an
application-defined `Report` class. Suppose now we also wish to
represent users as instances of a `User` class, and that there's
a privileged `SuperUser` subclass, say:

```python
class User:
    ...
class SuperUser(User):
    ...
```

These classes may *also* be used as specializers, so we might have
rules like:

```polar
allow(actor: User, "GET", resource: Report{author: actor});
allow(actor: SuperUser, "GET", resource: Report);
allow(actor: SuperUser, "PUT", resource: Report);
allow(actor: SuperUser, "DELETE", resource: Report);
```

If we like we can abstract actions as well, to group them, say:

```polar
allow(actor: User, action: Read, resource: Report{author: actor});
allow(actor: SuperUser, action: Access, resource: Report);
```

Here we have a generic rule, `allow`, with specific implementations (rules,
or "methods" if you like) that specialize on all of its arguments' types.
This is an extremely flexible framework for expressing complex logic
concisely as code.

Conclusion
----------
So we've come back around to view rules as code again. As code, they have
strict execution semantics that must agree with those of the application.
But we have also seen them as data, encoding permissions directly and
eschewing the complexities of a general language. To handle complex
authorization policies efficiently, we must view rules as neither one nor
the other per se, but rather optimize for both viewpoints simultaneously.
