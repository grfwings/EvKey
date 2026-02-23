# EvScript Language Specification

Version 2.0

## Overview

EvScript is a small, simple language for creating keyboard & mouse macros (replayable automation scripts) in EvKey. The language is designed to be human-readable and writable, making it easy to create, edit, and understand macro scripts.

## Core Concepts

### Events

The **event** is the basic unit of EvScript. Most EvScript events are essentially a 1:1 representation of a [libevdev](https://www.freedesktop.org/software/libevdev/doc/latest/) event. There are currently 6 supported event types:

#### Hold

Hold a key or set of keys for a duration:

```rust
hold W for 1000
hold SPACE for 50
hold BTN_LEFT for 100
```

#### Tap

Press and release a key with a short default duration (50ms). This is a shortcut for `hold KEY for 50`:

```rust
tap SPACE
tap ENTER
tap BTN_LEFT
```

#### Wait

Pause execution for a duration:

```rust
wait 1000
wait 50
```

#### Scroll

Scroll the mouse wheel. Scroll is an instant action and cannot be used inside sets.

```rust
scroll down 3    // Scroll down 3 units
scroll up 1      // Scroll up 1 unit
scroll left 2    // Scroll left 2 units
scroll right 4   // Scroll right 4 units
```

To combine scrolling with key holds, use a procedure:
```rust
let scroll_while_moving = [
  hold W for 500;
  scroll down 1;
  hold W for 500;
]
```

#### Move

Move the mouse cursor relative to current position. Move is an instant action and cannot be used inside sets.

```rust
move 100 50    // Move right 100px, down 50px
move -50 -50   // Move left 50px, up 50px
```

#### Run

Execute a procedure:

```rust
run procedure_name;
run procedure_name(arg1, arg2);
```

### Time Units

All time values are in **milliseconds**. No suffix is required.

```rust
wait 1000      // Wait 1000 milliseconds (1 second)
hold W for 50  // Hold W key for 50 milliseconds
```

### Definition Types

EvScript supports three types of definitions:

1. **Constants** - Numeric values for reuse
2. **Sets** - Simultaneous actions (multiple keys held at the same time)
3. **Procedures** - Sequences of events executed in order

### Comments

Comments begin with `//` and extend to the end of the line. They can appear on their own line or after a statement. Comments are stripped during lexing and are not part of the grammar.

```rust
// This is a comment
wait 100;  // This is also a comment
```

### Reserved Keywords

The following words cannot be used as identifiers: `let`, `hold`, `tap`, `wait`, `move`, `scroll`, `run`, `for`, `up`, `down`, `left`, `right`.

## Syntax

### Constants

Define numeric constants for reuse throughout your macro:

```rust
let TAP_TIME = 50;
let WALK_DURATION = 2000;
```

### Sets

Sets represent simultaneous key holds - multiple keys held at the same time. Sets can ONLY contain `hold` actions.

**Shorthand syntax** (most common):
```rust
let diagonal = hold { W, S };
let attack = hold { BTN_LEFT, W };
```

**Full syntax** (equivalent):
```rust
let diagonal = { hold W, hold S };
let combo = { hold CTRL, hold SHIFT, hold A };
```

**Anonymous sets** (use without defining):
```rust
hold { W, S } for 1000;
{ hold W, hold SHIFT } for 500;
```

Sets require a duration when used:
```rust
diagonal for 1000;
```

### Procedures

Procedures are sequences of statements executed in order. Each statement in a procedure must end with a semicolon.

```rust
let gather = [
  hold W for 2000;
  scroll down 1;
  wait 250;
  hold F for 600;
]
```

Run procedures with the `run` keyword:
```rust
run gather;
```

### Parameters

Both sets and procedures can accept parameters:

```rust
let strafe(key, duration) = [
  hold key for duration;
  wait 100;
];

let multi_hold(key1, key2) = hold { key1, key2 };

// Usage
run strafe(D, 5000);
multi_hold(W, SHIFT) for 1000;
```

Constants can be used as arguments:
```rust
let HOLD_TIME = 500;

let hold_and_wait(key) = [
  hold key for HOLD_TIME;
  wait 100;
];
```

## Key Names

Keys are referenced by their Linux keycode names in uppercase:

**Letters**: `A`, `B`, `C`, ... `Z`

**Numbers**: `0`, `1`, ... `9`

**Modifiers**: `SHIFT`, `CTRL`, `ALT`, `SUPER`

**Special keys**: `SPACE`, `ENTER`, `ESC`, `TAB`, `BACKSPACE`

**Function keys**: `F1`, `F2`, ... `F12`

**Mouse buttons**: `BTN_LEFT`, `BTN_RIGHT`, `BTN_MIDDLE`

See `src/keymap.rs` for the complete list of supported keys.

## Complete Example

```rust
// EvKey Macro
// Layout: QWERTY

// Constants for timing
let GATHER_DURATION = 2000;
let STRAFE_TIME = 500;

// Reusable key combinations
let diagonal_forward = hold { W, S };
let diagonal_back = hold { A, S };
let attack_move = { hold W, hold BTN_LEFT };

// Procedures
let gather = [
  hold W for GATHER_DURATION;
  scroll down 1;
  wait 250;
  hold F for 600;
];

let strafe(direction, duration) = [
  hold direction for duration;
  wait 100;
];

let combo_attack = [
  attack_move for 1000;
  tap SPACE;
  wait 500;
  diagonal_forward for 2000;
];

// Main macro script
wait 2000;
run gather;
wait 1000;
diagonal_forward for STRAFE_TIME;
run strafe(D, 6444);
run combo_attack;
hold { W, SHIFT } for 3000;
```

## Semantic Rules

### Type System

1. **Constants** are numbers and can be used wherever a duration or numeric argument is expected
2. **Sets** must be followed by `for duration` when used as actions
3. **Procedures** must be invoked with the `run` keyword

### Parameter Type Checking

Parameters are type-checked at their usage site. Each parameter receives a type based on how the argument is used:
- A parameter used after `hold` or `tap` must receive a **key**
- A parameter used after `for` or `wait` must receive a **number** (or constant)
- Argument count must match parameter count

Passing the wrong type is a compile-time error:
```rust
let my_macro(key) = [
  hold key for 1000;
];

// OK - D is a key
run my_macro(D);

// ERROR - 500 is a number, not a key
run my_macro(500);
```

### Scoping

- Top-level definitions are global
- Definitions inside procedures are locally scoped and may shadow global definitions, including key names
- Procedures may be defined inside other procedures (locally scoped)
- Forward references are allowed
- Each identifier can only be defined once per scope (no redefinition within the same scope)

### Composition

Definitions can reference other definitions:

```rust
let base = hold { W, S };

let advanced = [
  base for 1000;
  wait 500;
];

let super_advanced = [
  run advanced;
  run advanced;
];
```

### Recursion

Recursion is not allowed. A definition cannot reference itself, directly or indirectly.

```rust
// INVALID - direct recursion
let loop = [
  run loop;
]

// INVALID - indirect recursion
let a = [ run b; ]
let b = [ run a; ]
```

## Grammar Reference

```ebnf
file          ::= (statement ";")*

statement     ::= definition | action

definition    ::= "let" (identifier | const_name) params? "=" value

value         ::= number          // constant
                | set             // set of simultaneous key holds
                | sequence        // procedure

set           ::= "hold" "{" key_list "}"
                | "{" hold_list "}"

sequence      ::= "[" statement_list "]"

key_list      ::= key ("," key)* ","?

hold_list     ::= "hold" key ("," "hold" key)* ","?

statement_list ::= (statement ";")*

action        ::= "hold" key "for" duration
                | "hold" "{" key_list "}" "for" duration
                | "{" hold_list "}" "for" duration
                | "tap" (key | identifier)
                | "wait" duration
                | "move" int int
                | "scroll" direction number
                | "run" (identifier | const_name) args?
                | (identifier | const_name) args? "for" duration

duration      ::= number | identifier | const_name

params        ::= "(" identifier ("," identifier)* ","? ")"

args          ::= "(" expression ("," expression)* ","? ")"

expression    ::= number | identifier | const_name | key

direction     ::= "up" | "down" | "left" | "right"

number        ::= [0-9]+

int           ::= "-"? number

comment       ::= "//" [^\n]*

key           ::= [A-Z_][A-Z0-9_]* | [0-9]

identifier    ::= [a-z_][a-z0-9_]*

const_name    ::= [A-Z][A-Z0-9_]*

reserved      ::= "let" | "hold" | "tap" | "wait" | "move" | "scroll"
                | "run" | "for" | "up" | "down" | "left" | "right"
```
