;; No-op module that returns immediately
(module
  (func $noop (export "_start")
    ;; Do nothing, just return
  )
  (memory (export "memory") 1)
)