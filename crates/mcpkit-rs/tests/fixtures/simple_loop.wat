;; Simple loop that consumes fuel
(module
  (func $loop (export "_start")
    (local $i i32)
    (local.set $i (i32.const 0))
    (loop $continue
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $continue (i32.lt_u (local.get $i) (i32.const 1000)))
    )
  )
  (memory (export "memory") 1)
)