;; Module that writes to stdout
(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))

  (memory (export "memory") 1)
  (data (i32.const 8) "Hello, World!\n")

  (func $start (export "_start")
    ;; iov_base = 8, iov_len = 14
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 14))

    ;; fd_write(stdout=1, iovs=0, iovs_len=1, nwritten=28)
    (drop
      (call $fd_write
        (i32.const 1)  ;; stdout
        (i32.const 0)  ;; iovs
        (i32.const 1)  ;; iovs_len
        (i32.const 28) ;; nwritten
      )
    )
  )
)