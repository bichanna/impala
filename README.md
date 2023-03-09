<div align="center">
    <h1>The Impala Programming Language</h1>
    |
    <a href="https://bichanna.github.io/impala-book/">Doc</a>
    |
</div><br>

<div align="center">
</div>

**IMP**ure function**A**l **LA**nguage is a high-level, dynamically- and strongly-typed, functional programming language that runs on a virtual machine.
**Impala** is my highschool Computer Science project written in Rust.

I'm sorry for the ugly and inefficient and awful code. That's because Impala is my first serious project.

## Examples

```js
range := import("range")
std := import("std")

// bubble sort
func bubble_sort!(list)
    range.range(0, len(list)) |> range.each() <~ (i, _)
        range.range(0, len(list) - i - 1) |> range.each() <~ (j, _)
            std.if(list.j > list.(j+1)) <~ 
                [list.j, list.(j+1)] = [list.(j+1), list.j]

list := [-2, 4, 2, 1, 0, 5, -1, 6]
bubble_sort!(list)
println(list) // [-2, -1, 0, 1, 2, 4, 5, 6]
```

```js
std := import("std")
fmt := import("fmt")

// fibonacci
func fib(n) n <= 1 : n ? fib(n-1) + fib(n-2)

std.range(0, 10) |> std.each() <~ (i, _)
    fmt.println("fib({}) = {}", i, fib(i))
```
