;; Infinite loop that will exceed fuel limit
(module
  (func $infinite (export "_start")
    (loop $forever
      (br $forever)
    )
  )
  (memory (export "memory") 1)
)