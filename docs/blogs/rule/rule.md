Anatomy of a Rule
=================

Abstract
--------
The duality between code & data has exploitable consequences in the context
of authorization rules. Sometimes a rule expresses a piece of data directly,
such as who may do what to which resource. Other times a rule expresses logic,
or code: this actor may do that to some resource *if* certain other conditions
are also met, such as the actor or resource belonging to some model type. To
handle complex authorization policies, a system should handle both points of
view efficiently and naturally.

Rules as Data
-------------
An authorization policy specifies the requirements for authorization as *rules*.
The rules may be in the form of permissions *data*, like an ACL: a (long) list
of who can do what. In its simplest form, this may be only a few bits per rule,
such as in traditional Unix file permissions. Or it may come in the form of a
matrix like this one:

```
allow,alice,read,/reports/alice/
allow,alice,read,/reports/bob/
allow,bhavik,read,/reports/bhavik/
allow,bob,read,/reports/bob/
allow,marjory,read,/reports/alice/
allow,marjory,read,/reports/bhavik/
allow,marjory,read,/reports/bob/
allow,marjory,read,/reports/marjory/
```

This matrix encodes which users of a system may access what paths.
Here we've chosen to represent the matrix as CSV, but the encoding
is irrelevant; this kind of authorization data is easily encoded in
any authorization system, or handled directly by application code.

Another way to encode this kind of matrix is as a list of simple
rule definitions, one per row:

```
allow("alice", "read", "/reports/alice/");
allow("alice", "read", "/reports/bob/");
allow("bhavik", "read", "/reports/bhavik/");
allow("bob", "read", "/reports/bob/");
allow("marjory", "read", "/reports/alice/");
allow("marjory", "read", "/reports/bhavik/");
allow("marjory", "read", "/reports/bob/");
allow("marjory", "read", "/reports/marjory/");
```

Here the encoding is the oso rule language, Polar, but again the encoding
is not really germane. The point is just that what we previously thought
of as data has now become code in a declarative programming language.
It's not very interesting code, but it is simple and direct, and makes
an easy target for translation from nearly anything else. We'll come back
and make it more interesting shortly, but let's take it on its own terms
for now.

What happens as the permissions matrix grows in size? Even with naive
algorithms, a small matrix should pose no performance problem. But as
with any other type of data, the tools needed to cope with it vary as
the data grows. If the list is very large or if frequent dynamic updates
are required, some kind of database is going to be most appropriate.
But for a moderately sized, mostly-read-only but data-intensive policy,
an authorization system ought to be able to handle the job itself.

To handle largish data-heavy policies with Polar, we implemented a simple
indexing scheme over constant data. We have seen it deliver very large
speedups (multiple orders of magnitude) in authorization decisions on
certain (realistic) policies. The speedups come from eliminating most
or all rules from consideration up front with just a few hash lookups.
We call it the *pre-filter*, since its job is to keep the "main" filter —
the general rule selection and matching process we'll discuss shortly —
from even considering most rules. It is able to do its job by considering
the parts of rules purely as data, while still respecting the semantics
of the rules as code.

Rules as code
-------------
For complex authorization policies, a pure "policy as data" strategy is
simply not viable [citation needed]. But data used to make authorization
decisions is never random, and so almost always has exploitable structure.
As usual in technology, we can exploit that structure through *abstraction*.

Let's look again at our simple permissions matrix. One simple pattern
that should be evident is captured by the informal rule "everyone may
read their own reports". We can capture this formally in Polar as:

```
allow(actor: String, "read", resource: String) if
    resource.split("/") = ["", "reports", actor, ""];
```

This rule is universally quantified over all string-valued actors and
resources, and so replaces a potentially infinite set of data rules.
If we wish to quantify over more specific classes of actors and resources
(e.g., from our application), we can refine the type restrictions:

```
allow(actor: Reporter, "read", resource: Report) if
    resource.path.split("/") = ["", "reports", actor, ""];
```

This allows extremely fine-grained decisions without a large blowup in
policy size. It is also easy to add additional semantics:

```
allow(actor: Reporter, "read", resource: Report) if
    resource.author = actor;
```

These kinds of rules behave very differently than the data rules we saw
earlier. They may have data embedded in them (e.g., the `"read"` action,
the `"reports"` path segment), but they are inherently *code*. These rules
are *executed* or *called* with a list of arguments as part of an
authorization query, not just *matched* as data. They may contain arbitrary
logical expressions, which are encoded here as Horn clauses, but once again
the encoding is inessential — the essential feature is the *interpretation*
process, where the rules are treated as meaning-bearing expressions of a
logical language rather than opaque or "quoted" data. Types or classes
in such a language denote structured sets of data, semantically related
through subtyping. Decades of software development has shown that these
kinds of abstractions can enable immense compression of complex code,
with corresponding advantages in efficiency and maintainability.

Nothing comes for free, of course. The cost of abstraction in authorization
is really no different than its cost anywhere else:

1. The abstractions may be inappropriate for the data.
2. As the structure of the data changes, the abstractions must be adjusted to match.
3. The abstraction itself may be expensive in time or space.

The first two need no further commentary; code has bugs, requirements
change. The third is an implementation issue, and leads to many interesting
sub-problems.

In the case of Polar, for instance, the use of type restrictions for rule
parameters leads to the notions of rule *applicability* and *sorting*.
For a given set of arguments, only rules whose parameters *match* the
corresponding arguments — in structure, type, or both — have their bodies
executed. Rules that do not match need not be considered, and so are
*filtered* out as part of the calling sequence.

After the applicable rules have been selected, we must then decide in what
order to execute them. Sometimes it doesn't matter, but in the presence of
exceptions and overrides for specific classes, it can. Polar therefore
*sorts* the applicable rules by specificity, and executes the most specific
rule first. This allows more specific rules to selectively override less
specific ones, which can be a source of considerable expressive power when
you need it.

This filtering and sorting process is relatively slow compared to an index
lookup. This is what makes the pre-filter so effective in speeding up
certain kinds of policies; the fewer rules that need to be selected for
applicability and sorted, the faster the call sequence executes. But in
exchange for a somewhat expensive calling sequence, we can further leverage
available abstractions over our data, namely the types in our model,
with very little code.

Rules Redux
-----------
We have now seen authorization rules from two points of view.
Viewed as code, rules are interpreted or run, and so have an essentially
dynamic character. Viewed as data, they may be indexed ahead of time,
thus exploiting their static characteristics. So which is it? Are
rules code or data, fish or fowl?

The answer of course, is both: code and data are dual, and no one
point of view is primary. But switching points of view can sometimes
lead us to opportunities for optimization that may be hard to see
from the other.
