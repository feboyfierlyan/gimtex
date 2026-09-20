# Project structure

```
.
├── main.rs
└── math.rs
```

# File contents

## File: main\.rs (42 tokens)

```
   1 | mod math;
   2 | 
   3 | fn main() {
   4 |     println!("The answer is {}", math::double(21));
   5 | }
```

## File: math\.rs (31 tokens)

```
   1 | pub fn double(value: i32) -> i32 {
   2 |     value * 2
   3 | }
```

