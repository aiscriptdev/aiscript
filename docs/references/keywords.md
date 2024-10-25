## AI

### ai

Use `ai` keword to define an AI function and call AI functions.

```rs
ai fn sentiment(text: str) -> float {
    prompt "Analyze the sentiment of the following text: {text}"
}

let value = sentiment("I love AiScript")
print(value) # 0.9
```

### prompt

`prompt` is used to ask AI for a response with the given prompt.

```py
let a = prompt "What is AI?";
print a;
```

`prompt` supports customizations, the format is `<company>://<model>?<key1>=<value1>&<key2>=<value2>`. For example:

```py
let a = prompt "openai://gpt-3.5-turbo?temperature=1 What is Rust?";
print a;

let b = prompt "anthropic://claude-3-5-sonnet-20241022?temperature=1 What is Rust?";
print b;
```

### embedding

### agent

```rs
agent Researcher {
    use tool.GoogleSearch
    use tool.Wikipedia

    role => "Researcher"
    goal => "You are a researcher. Your task is to find information about the following"
    backstroy => "You have access to the following tools: {tools}"
    verbose => true
}
```

### task

```rs
task A {
    use agent.Researcher

    use tool.GoogleSearch

    description => "Research the following topic: {topic}"
    expected => "You have found the following information: {result}"
}
```

## Language

### and

Binary operator for logical conjunction. Returns `true` if both operands are `true`. `and` is short-circuiting, meaning that the second operand is not evaluated if the first operand is `false`.

```py
print(true and false) # false
print(true and true) # true
print(false and true) # false, second operand is not evaluated
```

### break

Breaks out of the innermost loop.

```rs
for i in 1..10 {
    if i == 5 {
        break
    }
    print(i)
}
```

### catch

`catch` is used to handle errors.

### const

Declares a constant. Constants can only be declared once and must be initialized with a value, they cannot be reassigned.

```rs
const PI = 3.14
```

### continue

Continues to the next iteration of the innermost loop.

```rs
for i in 1..10 {
    if i % 2 == 0 {
        continue
    }
    print(i)
}
```

### else

`else` is used to specify a block of code to be executed if the condition in the `if` statement is `false`.

```rs
if condition1 {
    // do something1
} else if condition2 {
    // do something2
} else {
    // do something else
}
```

### enum

```rs
enum Color {
    Red,
    Green,
    Blue,
}
```

### false

`false` is a boolean literal that represents the logical value `false`.

### fn

`fn` is used to declare a function.

```rs
fn add(a: int, b: int) -> int {
    return a + b
}
```

### for

### if

`if` is used to specify a block of code to be executed if a condition is `true`.

```rs
if condition1 {
    // do something1
else if condition2 {
    // do something2
} else {
    // do something else
}
```

### in

`in` has two use cases:

- iterate over a list, a map, or a range.
- check if a value is in a list, a map, or a range.

```rs
let numbers = [1, 2, 3, 4, 5]
for number in numbers {
    print(number)
}

let person = {
    "name": "AiScript",
}
print("name" in person) # true
print("age" in person) # false
print(1 in numbers) # true
print(0 in numbers) # false
```

### let

`let` is used to declare a variable.

```rs
let name = "AiScript"
```

### match

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

### not

`not` is a unary operator that returns the opposite of its operand. In other languages, it often use the `!` symbol, but in AiScript, we use the `not` keyword.

```
print(not true) # false
print(not false) # true
```

### or

Binary operator for logical disjunction. Returns `true` if either of the operands is `true`. `or` is short-circuiting, meaning that the second operand is not evaluated if the first operand is `true`.

```py
print(true or false) # true, second operand is not evaluated
print(true or true) # true
print(false or true) # true
```

### or raise

`or raise` is used to raise an error if the previous return value is error.

```rs
with Tweet {
    fn create(user_id: int, content: str) -> Tweet, !SqlError {
        return sql {
            INSERT INTO tweet (user_id, content)
            VALUES (:user_id, :content) RETURNING *
        } or raise !SqlError("failed to create tweet")
    }
}
```

### raise

`raise` is used to return an error from a function.

```rs
fn divide(a: int, b: int) -> int, !DivideByZero {
    if b == 0 {
        raise !DivideByZero
    }
    return a / b
}
```

### return

Return a value from a function. If no value is returned, `nil` is returned.

`return` can be used to return multiple values from a function.

```rs
fn add(a: int, b: int) -> int {
    return a + b
}

fn add_and_subtract(a: int, b: int) -> (int, int) {
    return a + b, a - b
}
```

### struct

```rs
struct Point {
    x: int
    y: int
}

let p = Point { x: 1, y: 2 }
```

### static

### true

### try

`try` is used to handle errors.

```py
fn div(a: int, b: int) -> int, !DivByZero {
    if b == 0 {
        raise !DivByZero
    }
    return a / b
}

# handle error
let e = try div(10, 0) catch err {
    raise err
}

# syntax sugar to try method() catch err
let e = div(10, 0)?

# handle error late
let r = try div(10, 0) catch err {
    match err {
        !DivByZero => print("DivByZero"),
        !UnknownError => print("UnknownError"),
        _ => print("UnknownError"),
    }
}
```

`true` is a boolean literal that represents the logical value `true`.

### use

`use` is used to import a module.

## Route

### body

### delete

### get

### path

### post

### put

### query

### route

## Model

### model

### auto

### with

### sql

Execute a SQL query. `sql` is a keyword that takes a SQL query as a string and returns the result of the query. The syntax to bind parameters to the query is `:name`.

```py
with Tweet {
    fn create(user_id: int, content: str) -> Tweet {
        return sql {
            INSERT INTO tweet (user_id, content)
            VALUES (:user_id, :content) RETURNING *
        }
    }
}
```

## Schema

### schema
