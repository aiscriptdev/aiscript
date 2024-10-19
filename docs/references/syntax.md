AiScript is a programming language for building web applications. It is inspired by Python, Rust and JavaScript.

## Features

- Dynamic typing interpreter language
- Auto garbage collection
- High level and domain specific in web development
- Function is first class citizen
- No semicolon end of line
- Rich standard library
- High performance, the interpreter is written in Rust

## Comments

AiScript use `#` as the comment symbol.

```py
# This is a comment

# A hello API endpoint
get /hello {
    query {
        # The name of the person to say hello to
        name: str
    }

    return "Hello, {name}!"
}
```

Every comment on route will be generated to the OpenAPI documentation.

## Variables

```rs
let name = "AiScript"
let age = 18
```

## Constants

Constants are declared with the `const` keyword, and they are immutable. Constants can only be declared once and must be initialized with a value, they cannot be reassigned.

```rs
const PI = 3.14
```

Access constants syntax is `const.NAME`, the `const.` is required.

```py
get /hello {
    return "PI is {const.PI}"
}
```

## Data types

```rs
# nil
let nil_value = nil

# strings
let name = "AiScript"

# integers
let age = 18
let negative = -1
let hex = 0x10
let octal = 0o10
let binary = 0b10
let big = 1_000_000_000_000

# floats
let pi = 3.14

# booleans
let flag1 = true
let flag2 = false

# arrays
let numbers = [1, 2, 3, 4, 5]

# maps
let person = {
    "name": "AiScript",
    "age": 18,
    "is_male": true,
    "hobbies": ["reading", "coding", "gaming"],
    "address": {
        "city": "Beijing",
        "street": "No. 100, Xihuan Road",
        "zipcode": "100000",
        "country": "China",
        "phone": "13800138000",
    }
}

#  tuples
let person = ("AiScript", 18, true)

# set
let set = {1, 2, 3, 4, 5}

```

## Operators

```rs

# arithmetic operators
let a = 1 + 2
let b = 1 - 2
let c = 1 * 2
let d = 1 / 2
let e = 1 % 2

# logical operators
let a = true and false
let b = true or false
let c = not true

# comparison operators
let a = 1 == 2
let b = 1 != 2
let c = 1 > 2
let d = 1 < 2
let e = 1 >= 2
let f = 1 <= 2

# bitwise operators
let a = 1 & 2
let b = 1 | 2
let c = 1 ^ 2
let d = 1 << 2
let e = 1 >> 2

# assignment operators
let a = 1
a += 2
a -= 2
a *= 2
a /= 2
a %= 2

# ternary operator

# in operator
let a = 1 in [1, 2, 3]
let b = 1 not in [1, 2, 3]

# typeof operator
let a = typeof 1 # "int"
let b = typeof "1" # "string"
let c = typeof true # "bool"
let d = typeof [1, 2, 3] # "array"
let e = typeof {1, 2, 3} # "set"
let f = typeof {1: 1, 2: 2, 3: 3} # "map"
let g = typeof fn(a: int) -> int { return a } # "function"
let h = typeof nil # "nil"
```

## String

```rs
# string interpolation
let a = "AiScript"
let b = "Hello, {a}!"

# string concatenation
let a = "AiScript"
let b = "Hello, " + a + "!"

# string format
let a = "AiScript"
let b = "Hello, {}!"

# string length
let a = "AiScript"
let b = a.len()


```

## Slice

```rs
# array slice
let a = [1, 2, 3, 4, 5]
let b = a[0:3] # [1, 2, 3]
let c = a[3:] # [4, 5]
let d = a[:3] # [1, 2, 3]

# string slice
# string slice
let a = "AiScript"
let b = a[0:3] # "Web"
let c = a[3:] # "Script"
let d = a[:3] # "Web"

```

## Control flow

```rs
let age = 20

if age > 60 {
    print("You are a senior")
} else if age > 18 and age <= 60 {
    print("You are an adult")
} else if age > 12 and age <= 18 {
    print("You are a teenager")
} else {
    print("You are a child")
}
```

## Match

```rs
let language = "AiScript"

match language {
    "AiScript" => print("AiScript"),
    "Rust" => print("Rust"),
    "Python" => print("Python"),
    "JavaScript" => print("JavaScript"),
    _ => print("Unknown language"),
}
```

## Functions

```rs
fn add(a: int, b: int) -> int {
    return a + b
}
```

## Closures

```rs
fn add(a: int) -> fn {
    fn _add(b: int) -> int {
        return a + b
    }

    return _add
}

let add5 = add(5)
print(add5(10)) # 15

```

## Loops

AiScript only has one keyword for loops, the `for` keyword. You can use it to iterate over a list, a map, or a range. You can also use it achieve unconditional loops and `do while` loops.

### Iterate over a list

```rs
let numbers = [1, 2, 3, 4, 5]

for number in numbers {
    print(number)
}
```

### Iterate over a map

```rs
let person = {
    "name": "AiScript",
    "age": 18,
    "gender": "male",
}

for (key, value) in person {
    print("{key}: {value}")
}
```

### Iterate over range

```rs
for i in 1..10 {
    print(i)
}
```

### Unconditional loop

```py
for {
    # unconditional loop
    print("Hello, World!")
}
```

### Do while loop and break

```rs
let i = 0

for {
    if i > 10 {
        break
    }
    i += 1
}
```

### Continue

```py
for i in 1..10 {
    if i % 2 == 0 {
        continue
    }
    print(i)
}
```

## Struct

```rs
struct Person {
    name: string,
    age: int,
}

with Person {
    fn say_hello() {
        print("Hello, my name is :name and I'm :age years old")
    }

    static fn new(name: string, age: int) -> Person {
        return Person {
            name: name,
            age: age,
        }
    }
}

let person = Person::new("AiScript", 18)
person.say_hello()
```

## Error handling

Every type prefix with `!` is an error type. Use `raise` keyword to raise an error,

```py
fn div(a: int, b: int) -> int, !DivByZero {
    if b == 0 {
        raise !DivByZero
    }
    return a / b
}

# handle error
let e = try div(10, 0) -> err {
    raise err
}

# syntax sugar to try method() -> err
let e = div(10, 0)?

# handle error late
let r = try div(10, 0) -> err
match err {
    !DivByZero => print("DivByZero"),
    _ => print("UnknownError"),
}
```
