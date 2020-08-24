Anatomy of a Rule
=================

Abstract
--------
The duality between code & data has exploitable consequences in the context
of authorization rules. Sometimes a rule expresses a piece of data, such as
who may do what to which resource. Sometimes a rule expresses logic, or code:
this actor may do that to some resource *if* certain other conditions are
also met. To handle complex authorization policies, a system must handle
both points of view efficiently and naturally.

Introduction
------------
An authorization policy specifies the requirements for authorization.
Those requirements may be abstract; e.g., any user may see their own
reports. This is authorization *logic*, and suggests a view of rules
as *code*. But a policy may also be extremely concrete; e.g., Alice may
see Bob's reports in addition to her own, but not Bhavik's. This is *data*
about who may do what to which resources, and might not use any logical
abstractions. It could be encoded as an ACL or a permissions matrix:

```
allow,alice,GET,/reports/alice/
allow,alice,GET,/reports/bob/
allow,bhavik,GET,/reports/bhavik/
allow,bob,GET,/reports/bob/
allow,marjory,GET,/reports/alice/
allow,marjory,GET,/reports/bhavik/
allow,marjory,GET,/reports/bob/
allow,marjory,GET,/reports/marjory/
```

Here we've represented the matrix as CSV, but the encoding is irrelevant;
this kind of authorization data is easily encoded in any authorization system,
or handled directly by application code. But what happens when the matrix
gets large? Application code then needs to worry about making lookups over
large data sets fast to provide timely authorization. It may be able to
offload that task to a database, but it can then be difficult to adapt the
data to new authorization requirements. What if organizational structure
changes, or new services are added — what happens when a new logging or
auditing system needs access to every service? These kinds of schema
migrations can be difficult and errror-prone, especially as the size of
the policy grows.

One way to manage this kind of complexity is to use abstraction to refactor
the policy. Are there [roles] that can be factored out that reflect latent
structure in the data, e.g., relations between the actors/actions/resources
that the data represents? Perhaps there are common [attributes] of their
representations that can be used to group authorization decisions. Finally,
we can use [application domain models as types] to capture commonalities
among sets of data, allowing us to organize and exploit these commonalities
in type-directed code. These kinds of abstractions can enable immense
compression of policy code, with corresponding advantages in efficiency and
maintanability.

Rules as data
-------------
But let's start with the data. Abstraction is well and good in the
abstract, but in concrete authorization problems there's often just
some big matrix of permissions to start with, like the one we showed
above (but bigger). Perhaps it has been imported from an identity
provider, or another authorization system. Such cases must be
supported efficiently.

So that we have a concrete example to work with, we'll encode the
permissions matrix above into the oso rule language, Polar:

```
allow("alice", "GET", "/reports/alice/");
allow("alice", "GET", "/reports/bob/");
allow("bhavik", "GET", "/reports/bhavik/");
allow("bob", "GET", "/reports/bob/");
allow("marjory", "GET", "/reports/alice/");
allow("marjory", "GET", "/reports/bhavik/");
allow("marjory", "GET", "/reports/bob/");
allow("marjory", "GET", "/reports/marjory/");
```

This set of rule definitions is used by issuing a query like
`allow(actor, action, resource)`, with each of the three arguments
a string, presumably derived from the request being authorized.
Authorization is performed by *matching* or *failing* to match
a rule whose formal parameters (the stuff in parentheses) match
the supplied arguments. For instance, the query
`allow("alice", "GET", "/reports/bob/")`
would successfully match the second rule, but
`allow("alice", "GET", "/reports/bhavik/")`
would not match any rules, and so would fail.

But we would like it to fail quickly, and if the list of rules
is long, a linear scan over all of them could be quite slow.
Databases use precomputed indices to support efficient queries
over large data sets at the cost of slightly slower modifications;
since policy changes are probably rare compared to authorization
requests, that seems like a good tradeoff here.

We have implemented indexing of constant data in Polar rules, and
have seen it deliver very large speedups (multiple orders of magnitude)
in authorization decisions for data-heavy policies. The speedups come
from efficiently eliminating most or all rules from consideration
up front with just a few hash lookups. We call it our *pre-filter*,
since its job is to keep the "main" filter — the general rule selection
and matching process we'll discuss shortly — from even considering
most rules.

Rules with patterns
-------------------
Indexing is one way to reduce the cost of matching a set of values against
a large set of patterns. Another is to use a more expressive pattern language
than "values match themselves". For example, regular expressions can
concisely denote large sets of strings, and matching can be made reasonably
efficient and convenient. And indeed, regular expressions are heavily used
in authorization and access control languages for exactly these reasons.

Strict regular expressions, of course, have their limits (viz.,
regular languages). One common way of extending the base facility is
with *binding*, whereby a given *name* (or *backreference*) may be bound
to a portion of the matched data, and referred to elsewhere in the pattern
by name. Binding seems to be an essential feature of effective pattern
languages, since parts of a pattern are frequently referred to in other
parts, often more than once or recursively.

Rules as code
-------------
And so another strategy for expressing patterns of data in rules is to
use a language with named *variables* that may be bound to data *values*,
and to specify *conditions* over those variables using logical expressions.
Classical examples include Prolog (which of course Polar is a dialect of),
which uses [unification] as its basic matching operation. Unification
is a binding operator if either argument is an unbound variable, and an
*equality* operator when both are bound; hence it is indicated by `=`.
Together with instances created with a `new` operator and attribute
access via the `.` operator, we can express rules like this:

```
allow(actor, "GET", resource) if
    new PathMapper("/reports/{actor}/").map(resource).actor = actor;
```

This rule says that any actor may get their own reports, for appropriately
structured resource paths. Rules like this must be *filtered* by
applicability to a given set of arguments by *checking* that the
arguments fulfill all of the conditions specified by the rule.

Let's see how this works. Suppose the query is
`allow("alice", "GET", "/reports/alice/")`.
Matching the parameters with the arguments by unification, `actor`
is first bound to the string `"alice"`, the string `"GET"` unifies
with the string `"GET"` by equality, and the resource is bound to
`"/reports/alice"`. The body (the part after the `if`) then
destructures the value of `resource` and unifies the second
component of that path with the value of `actor`. In this case that
unification succeeds, and the query is successful. But if the resource
were `"/reports/bhavik/"`, the unification would not succeed, and
the rule would fail to match; that special case would need to be
encoded separately. Nevertheless, this single logical rule potentially
replaces infinitely many data rules; we do not need to specify exactly
who may access their own reports if we know that everyone may.

Another way to concisely denote large or infinite sets of objects
is with *types* or *classes*. Your application may already have a
`User` class that represents your users; let's suppose you also have
a `Resource` class with an `owner` field. Leveraging those gets us
rules like:

```
allow(actor: User, "GET", resource: Resource) if
    resource.owner = actor;
```

This rule says that any user may get any resource that they own.
These kinds of rules are most powerful in conjunction with others
that specialize on more or less specific types; e.g., we might
augment the above with:

```
allow(actor: Auditor, "GET", resource: Resource) if not resource.privileged;
allow(actor: AdminUser, action, resource: Resource);
allow(actor: SpecialUser, action, resource: SpecialResource);
allow(actor: BannedUser, action, resource) if cut and false;
```

To support *exceptions* and *overrides*, both of which are common in
complex authorization policies, it is essential to specify the *order*
in which multiple matching rules are applied. For instance, we would
want the last rule, which always fails and considers no other rules,
to be applicable *first* for actors of the `BannedUser` class, no matter
which other rules match, e.g., via superclasses of that class; we would
not want to accidentally grant access to a user that has been banned.

To support such semantics, we can either supply the rule ordering as
additional code, or rely on an automatic system that uses the relations
among the types of the parameters to automatically order the rules.
In Polar we use the latter strategy; the ordering is always most-to-least
specific with respect to the given arguments, in left-to-right order.

A final observation brings us back to data. Specificity of types is
determined by subtyping, and since subtyping relations are defined by
the application model, they are, from the point of view of the rules,
*data about (types of) data*. E.g., with respect to a particular instance
of `BannedUser`, say, the above set of rules can't be properly sorted
until we know whether a `BannedUser` is more or less specific than an
ordinary `User`. Their names hint at an answer, but only the class definitions
or their runtime representations provide a canonical answer. This situation
makes the sorting process interesting and challenging, but the details must
be left for another time.

Rules Redux
-----------
And so we see that even highly abstracted as code, rules are inherently
tied to the data over which they operate. They retain its essential
structure, and reflect its origins in representing sets of related
objects: the actors, actions, and resources that the policy is about.

Viewed as code, rules are subject to interpretation, which must be
reasonably fast but must also respect application semantics. Viewed as
data, they are subject to query, which must also be reasonably fast even
over large data sets. Realistic policies, of course, contain arbitrary
mixtures of the two, suggesting that any authorization system that wishes
to handle them efficiently must therefore support both points of view.
