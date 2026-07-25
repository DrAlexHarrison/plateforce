# plateforce

Force-plate kinetic analysis with a method registry.

Every computed quantity is bound to a named method variant carrying its citation,
its exact rule, its known bias and a status flag. Choosing a different variant is
an explicit act that appears in the output, not an invisible default.

## Why the registry exists

Two independent open-source implementations of the same named methods, run over the
same 240 countermovement jump trials, agree at r = 0.952 on jump height and r = 0.638
on time to takeoff.

Across 244 trials, 9 published onset rules and 10 published jump-height methods:

| quantity | spread across published methods |
|---|---|
| time to takeoff | median 0.335 s, 38% of its own value |
| jump height | median 3.51 cm |

For reference, the training intervention that dataset was collected to measure moved
jump height by 1.98 cm. The method choice moves the number further than the training did.

## Status

Pre-implementation. The registry is drafted and the software is not written.

## Layout

```
registry/   method definitions as data: rule, citation, status, bias, parameters
docs/       method rulings, schema, and the reasoning behind both
```

## Contributing

Read `CONVENTIONS.md` first. It is binding, and it is short.

Adding a method means adding a registry entry, not writing code.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
